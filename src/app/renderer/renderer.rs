use std::{collections::HashMap, num::NonZero, ops::Range};

use wgpu::RenderPass;

use crate::{
    app::{
        app_config::AppConfig,
        renderer::{
            AllocationCache, BufferChunks, DrawItem, DrawList, DrawListBuilder, DrawPacket,
            DrawPacketNew, GPUAllocationHandle, Instruction, RenderCategory, RenderError,
            RenderUpdateDelta, RenderUpdateError, UploadMeshJob, VMValue, VertexArenaError,
            VertexArenaSelector,
            gpu_allocator::{
                GPUAllocator, LocalTransformUploadJob, UploadIndexJob,
                vertex_arena::{GPUArena, StaticGPUBuffer},
            },
            pipeline::PipelineCollection,
        },
    },
    util::types::{GlobalTransform, LocalTransform, ModelVertex, PNUJWVertex, PNUVertex, VIndex},
    world::{
        camera::Camera,
        entity_manager::EntityHandle,
        instance_manager::{ArchetypeId, ArchetypeTable, InstanceHandle, InstanceManager},
        world::{DrawSet, RenderGroup, RenderView},
    },
};

pub(super) struct EngineRenderPass {
    label: String,
    categories: Vec<RenderCategory>,
}

impl EngineRenderPass {
    fn create_pass<'frame>(
        label: &'frame str,
        encoder: &'frame mut wgpu::CommandEncoder,
        view: &'frame wgpu::TextureView,
    ) -> Result<RenderPass<'frame>, wgpu::SurfaceError> {
        // TODO match on render cat OR add generics to method call
        // TODO: customize render pass output
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            depth_stencil_attachment: None, // TODO: depth stencil
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.3,
                        g: 0.3,
                        b: 0.7,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        Ok(render_pass)
    }

    fn new(label: &str, categories: Vec<RenderCategory>) -> Self {
        Self {
            label: label.to_owned(),
            categories,
        }
    }
}

struct VertexArenaCollection {
    index_arena: GPUArena<VIndex>,
    static_arena: GPUArena<PNUVertex>,
    skinned_arena: GPUArena<PNUJWVertex>,
    local_transform_arena: GPUArena<LocalTransform>,
}

impl VertexArenaCollection {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            index_arena: GPUArena::<VIndex>::new(device),
            static_arena: GPUArena::<PNUVertex>::new(device),
            skinned_arena: GPUArena::<PNUJWVertex>::new(device),
            local_transform_arena: GPUArena::<LocalTransform>::new(device),
        }
    }
}

pub struct InstanceDataCollector<'a> {
    pub offset_map: OffsetMap,
    pub global_transforms: Vec<&'a [GlobalTransform]>,
    pub gt_len: usize,
}

#[derive(Default)]
pub struct OffsetMap {
    pub a_postion_offset: u16,
    // other tables
}
impl OffsetMap {
    fn offset_of(&self, a_id: ArchetypeId) -> u16 {
        match a_id {
            ArchetypeId::Position => self.a_postion_offset,
        }
    }
}

impl<'a> InstanceDataCollector<'a> {
    fn new() -> Self {
        Self {
            gt_len: 0,
            offset_map: OffsetMap::default(),
            global_transforms: Vec::new(),
        }
    }
}

pub struct Renderer {
    allocations: Vec<u32>,
    vertex_arenas: VertexArenaCollection,
    global_transform_buffer: StaticGPUBuffer<GlobalTransform>,
    pub pipelines: PipelineCollection,
    passes: Vec<EngineRenderPass>,
    pub(super) groups: Vec<RenderGroup>,
    pub(super) entity_group_index: HashMap<EntityHandle, usize>,
    draw_packet: DrawPacket,
}

impl Renderer {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            allocations: Vec::new(),
            vertex_arenas: VertexArenaCollection::new(&config.device),
            global_transform_buffer: StaticGPUBuffer::<GlobalTransform>::new(&config.device),
            pipelines: PipelineCollection::new(config),
            passes: Vec::new(),
            groups: Vec::new(),
            entity_group_index: HashMap::new(),
            draw_packet: DrawPacket {
                pnu: HashMap::new(),
                pnujw: HashMap::new(),
            },
        }
    }
    pub fn add_pass(&mut self, label: String, categories: Vec<RenderCategory>) {
        self.passes.push(EngineRenderPass { label, categories });
    }
    fn write_draw_list<V: ModelVertex>(
        &self,
        view: &RenderView,
        draw_map: &mut HashMap<BufferChunks, Vec<DrawItem>>,
        instance_idx: u16,
    ) where
        Self: VertexArenaSelector<V>,
        DrawPacket: DrawListBuilder<V>,
    {
        let arena: &GPUArena<V> = self.get_arena();
        let vertex_buf_id = arena.chunk_id(&view.gpu_handle);
        let index_buf_id = if PNUVertex::is_indexed(view) {
            Some(self.vertex_arenas.index_arena.chunk_id(&view.gpu_handle))
        } else {
            None
        };
        let draw_list = draw_map
            .entry(BufferChunks {
                vertex: vertex_buf_id,
                index: index_buf_id,
            })
            .or_insert_with(|| vec![]);
        let (local_transform_range, _, _) = self
            .vertex_arenas
            .local_transform_arena
            .resolve(&view.gpu_handle);

        DrawPacket::write_list(
            view,
            arena,
            draw_list,
            instance_idx as u32,
            local_transform_range.start,
        );
    }

    fn cache_resolve_pnu(&self, view: &RenderView, draw_list: &mut Vec<DrawList>) -> AllocationCache
    where
        Self: VertexArenaSelector<PNUVertex>,
    {
        let arena: &GPUArena<PNUVertex> = self.get_arena();
        let vertex_buffer_id = arena.chunk_id(&view.gpu_handle);
        let index_buf_id = if PNUVertex::is_indexed(view) {
            Some(self.vertex_arenas.index_arena.chunk_id(&view.gpu_handle))
        } else {
            None
        };
        let buffer_chunks = BufferChunks {
            vertex: vertex_buffer_id,
            index: index_buf_id,
        };

        let draw_list_idx = draw_list
            .iter()
            .position(|l| l.chunks == buffer_chunks)
            .unwrap_or_else(|| {
                draw_list.push(DrawList {
                    chunks: buffer_chunks,
                    items: vec![],
                });
                draw_list.len() - 1
            });

        let (vertex_range, _, _) = arena.resolve(&view.gpu_handle);
        let index_range = index_buf_id
            .map(|_| self.vertex_arenas.index_arena.resolve(&view.gpu_handle))
            .map(|i| i.0);

        AllocationCache {
            index_range,
            vertex_range,
            chunks_index: draw_list_idx,
            lt_offset: 0,
        }
    }

    pub fn gen_draw_calls<'frame>(
        &'frame self,
        instance_manager: &'frame InstanceManager,
        packet: &mut DrawPacketNew,
        queue: &wgpu::Queue,
    ) {
        for group in self.groups.iter() {
            for view in group.views.iter() {
                if let Some(pnu) = &view.pnu_draws {
                    // if the alloc id is not chached, add a new cache, and a new DrawList value if necessary
                    if !packet
                        .pnu_cache
                        .contains_key(&view.gpu_handle.global_allocation_id)
                    {
                        let alloc_cache = self.cache_resolve_pnu(view, &mut packet.pnu);
                        packet
                            .pnu_cache
                            .insert(view.gpu_handle.global_allocation_id.clone(), alloc_cache);
                    }
                    // get the DrawList from the alloc cache and write to it
                    let alloc_cache = &packet.pnu_cache[&view.gpu_handle.global_allocation_id];
                    for (i, mesh_id) in pnu.mesh_ids.iter().enumerate() {
                        packet.pnu[alloc_cache.chunks_index].items.push(DrawItem {
                            lt_idx: 0,
                            instances: 0..0,
                            primitives: DrawSet::within(
                                &pnu.primtitive_ranges[i],
                                &alloc_cache.vertex_range,
                            ),
                            indices: alloc_cache.index_range.as_ref().map(|i| {
                                DrawSet::within(&i, alloc_cache.index_range.as_ref().unwrap())
                            }),
                        });
                    }
                }
            }
        }
    }

    pub fn gen_draw_calls_new<'frame>(
        &'frame self,
        instance_manager: &'frame InstanceManager,
        packet: &mut DrawPacket,
        queue: &wgpu::Queue,
    ) {
        let mut collector = InstanceDataCollector::new();

        instance_manager.pos.collect(&mut collector, 0);
        // instance_manager.next_table.collect(&mut collector, instance_manager.pos.len())
        // ... and so on

        // COPY GLOBAL TRANSFORMS
        {
            let global_transforms = collector.global_transforms;
            if global_transforms.is_empty() {
                return;
            }
            if let Some(mut buffer_view) = queue.write_buffer_with(
                &self.global_transform_buffer,
                0,
                NonZero::new((collector.gt_len * size_of::<GlobalTransform>()) as u64).unwrap(),
            ) {
                let mut offset: usize = 0;
                for pos_slice in &global_transforms {
                    buffer_view[offset..offset + pos_slice.len() * size_of::<GlobalTransform>()]
                        .copy_from_slice(bytemuck::cast_slice(pos_slice));
                    offset += pos_slice.len() * size_of::<GlobalTransform>();
                }
            }
        }
        for group in self.groups.iter() {
            for view in group.views.iter() {
                self.write_draw_list::<PNUVertex>(view, &mut self.draw_packet.pnu, 0);
            }
        }
    }

    pub(super) fn add_render_group(
        &mut self,
        views: Vec<RenderView>,
        instance_handle: InstanceHandle,
        is_indexed: bool,
    ) {
        self.groups
            .push(RenderGroup::new(instance_handle, views, is_indexed));
    }

    pub(super) fn get_global_alloc_id(&mut self) -> u32 {
        self.allocations.push(self.allocations.len() as u32);
        (self.allocations.len() - 1) as u32
    }

    pub fn update(
        &mut self,
        constants: Vec<VMValue>,
        ops: Vec<Instruction>,
        queue: &wgpu::Queue,
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        self.interpret(constants, ops, queue)
    }

    pub(super) fn upload_local_transform_data<'frame>(
        &mut self,
        job: Option<LocalTransformUploadJob>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        if let Some(job) = job {
            self.vertex_arenas
                .local_transform_arena
                .upload(job, queue)?;
        }
        Ok(())
    }

    pub(super) fn upload_indices<'frame>(
        &mut self,
        job: UploadIndexJob,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        self.vertex_arenas.index_arena.upload(job, queue)?;
        Ok(())
    }

    pub fn render_blank(&self, config: &AppConfig) -> Result<(), RenderError> {
        let output = config.surface.as_ref().unwrap().get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = config
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(format!("Render Encoder for {}", "blank").as_str()),
            });
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        config.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn render(
        &self,
        config: &AppConfig,
        camera: &Camera,
        draw_packet: &DrawPacket,
    ) -> Result<(), RenderError> {
        for pass in &self.passes {
            let output = config.surface.as_ref().unwrap().get_current_texture()?;
            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder =
                config
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some(format!("Render Encoder for {}", pass.label).as_str()),
                    });
            {
                let mut render_pass = EngineRenderPass::create_pass("pass", &mut encoder, &view)?;

                render_pass.set_bind_group(0, camera.get_bind_group(), &[]);
                render_pass.set_bind_group(
                    1,
                    self.vertex_arenas.local_transform_arena.get_bind_group(),
                    &[],
                );
                render_pass.set_vertex_buffer(1, self.global_transform_buffer.slice(..));
                for render_category in pass.categories.iter() {
                    match render_category {
                        RenderCategory::OpaqueStatic => {
                            let pipeline = &self.pipelines.opaque_static;
                            render_pass.set_pipeline(&pipeline.pipeline);
                            for draw_entry in draw_packet.pnu.iter() {
                                render_pass.set_vertex_buffer(
                                    0,
                                    self.vertex_arenas
                                        .static_arena
                                        .buffer_from_chunk_id(draw_entry.0.vertex)
                                        .slice(..),
                                );
                                if let Some(index_buf_id) = draw_entry.0.index {
                                    render_pass.set_index_buffer(
                                        self.vertex_arenas
                                            .index_arena
                                            .buffer_from_chunk_id(index_buf_id)
                                            .slice(..),
                                        wgpu::IndexFormat::Uint16,
                                    );
                                }
                                for draw in draw_entry.1.iter() {
                                    render_pass
                                        .set_immediates(0, bytemuck::cast_slice(&[draw.lt_idx]));
                                    if let Some(indices) = &draw.indices {
                                        render_pass.draw_indexed(
                                            indices.clone(),
                                            draw.primitives.start as i32,
                                            draw.instances.clone(),
                                        );
                                    } else {
                                        render_pass
                                            .draw(draw.primitives.clone(), draw.instances.clone());
                                    }
                                }
                            }
                        }
                        RenderCategory::OpaqueSkinned => {
                            let pipeline = &self.pipelines.opaque_skinned;
                            render_pass.set_pipeline(&pipeline.pipeline);
                            for draw_entry in draw_packet.pnujw.iter() {
                                render_pass.set_vertex_buffer(
                                    0,
                                    self.vertex_arenas
                                        .skinned_arena
                                        .buffer_from_chunk_id(draw_entry.0.vertex)
                                        .slice(..),
                                );
                                if let Some(index_buf_id) = draw_entry.0.index {
                                    render_pass.set_index_buffer(
                                        self.vertex_arenas
                                            .index_arena
                                            .buffer_from_chunk_id(index_buf_id)
                                            .slice(..),
                                        wgpu::IndexFormat::Uint16,
                                    );
                                }
                                for draw in draw_entry.1.iter() {
                                    render_pass
                                        .set_immediates(0, bytemuck::cast_slice(&[draw.lt_idx]));
                                    if let Some(indices) = &draw.indices {
                                        render_pass.draw_indexed(
                                            indices.clone(),
                                            draw.primitives.start as i32,
                                            draw.instances.clone(),
                                        );
                                    } else {
                                        render_pass
                                            .draw(draw.primitives.clone(), draw.instances.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            config.queue.submit(std::iter::once(encoder.finish()));
            output.present();
        }
        Ok(())
    }
}
impl VertexArenaSelector<PNUJWVertex> for Renderer {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<PNUJWVertex>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        let _handle = self.vertex_arenas.skinned_arena.upload(mesh_job, queue)?;
        Ok(())
    }

    fn get_arena(&self) -> &GPUArena<PNUJWVertex> {
        &self.vertex_arenas.skinned_arena
    }
}

impl VertexArenaSelector<PNUVertex> for Renderer {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<PNUVertex>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        let _handle = self.vertex_arenas.static_arena.upload(mesh_job, queue)?;
        // TODO handle?
        Ok(())
    }

    fn get_arena(&self) -> &GPUArena<PNUVertex> {
        &self.vertex_arenas.static_arena
    }
}
