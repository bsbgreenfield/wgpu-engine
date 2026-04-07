use std::{collections::HashMap, num::NonZero, ops::Range};

use wgpu::RenderPass;

use crate::{
    app::{
        app_config::AppConfig,
        renderer::{
            Instruction, RenderError, RenderUpdateDelta, RenderUpdateError, UploadMeshJob, VMValue,
            VertexArenaError, VertexArenaSelector,
            gpu_allocator::{
                GPUAllocator, LocalTransformUploadJob,
                vertex_arena::{GPUArena, StaticGPUBuffer},
            },
            pipeline::PipelineCollection,
        },
    },
    util::types::{GlobalTransform, LocalTransform, ModelVertex, PNUJWVertex, PNUVertex},
    world::{
        camera::Camera,
        components::ComponentData,
        instance_manager::{InstanceHandle, InstanceManager},
        world::{DrawSet, RenderGroup, RenderView},
    },
};

pub enum RenderCategory {
    OpaqueStatic,
    OpaqueSkinned,
}

#[derive(Debug)]
pub struct DrawItem {
    lt_idx: u32,
    instances: Range<u32>,
    primitives: Range<u32>,
    // TODO: indices
}

#[cfg(test)]
impl DrawItem {
    pub fn get_lt_idx(&self) -> u32 {
        self.lt_idx
    }

    pub fn get_instances(&self) -> Range<u32> {
        self.instances.clone()
    }
    pub fn get_primitives(&self) -> Range<u32> {
        self.primitives.clone()
    }
}

trait DrawListBuilder<V: ModelVertex> {
    fn write_list(
        view: &RenderView,
        arena: &GPUArena<V>,
        draw_list: &mut Vec<DrawItem>,
        instance_idx: u32,
        lt_offset: u32,
    );
}

impl DrawListBuilder<PNUVertex> for DrawPacket {
    fn write_list(
        view: &RenderView,
        arena: &GPUArena<PNUVertex>,
        draw_list: &mut Vec<DrawItem>,
        instance_idx: u32,
        lt_offset: u32,
    ) {
        for (i, mesh_id) in view.pnu_draws.as_ref().unwrap().mesh_ids.iter().enumerate() {
            let (alloc_range, _, _) = arena.resolve(&view.gpu_handle);
            let prim_range = DrawSet::within(
                &view.pnu_draws.as_ref().unwrap().primtitive_ranges[i],
                &alloc_range,
            );
            draw_list.push(DrawItem {
                lt_idx: lt_offset + mesh_id,
                instances: instance_idx..instance_idx + 1,
                primitives: prim_range,
            });
        }
    }
}

impl DrawListBuilder<PNUJWVertex> for DrawPacket {
    fn write_list(
        view: &RenderView,
        arena: &GPUArena<PNUJWVertex>,
        draw_list: &mut Vec<DrawItem>,
        instance_idx: u32,
        lt_offset: u32,
    ) {
        for i in 0..view.pnujw_draws.as_ref().unwrap().mesh_ids.len() {
            let (alloc_range, _, _) = arena.resolve(&view.gpu_handle);
            let prim_range = DrawSet::within(
                &view.pnujw_draws.as_ref().unwrap().primtitive_ranges[i],
                &alloc_range,
            );
            draw_list.push(DrawItem {
                lt_idx: lt_offset,
                instances: instance_idx..instance_idx + 1,
                primitives: prim_range,
            });
        }
    }
}

pub struct DrawPacket {
    pnu: HashMap<usize, Vec<DrawItem>>,
    pnujw: HashMap<usize, Vec<DrawItem>>,
}

#[cfg(test)]
impl DrawPacket {
    pub fn get_pnu(&self) -> &HashMap<usize, Vec<DrawItem>> {
        &self.pnu
    }

    pub fn get_pnujw(&self) -> &HashMap<usize, Vec<DrawItem>> {
        &self.pnujw
    }
}

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
    static_arena: GPUArena<PNUVertex>,
    skinned_arena: GPUArena<PNUJWVertex>,
    local_transform_arena: GPUArena<LocalTransform>,
}

impl VertexArenaCollection {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            static_arena: GPUArena::<PNUVertex>::new(device),
            skinned_arena: GPUArena::<PNUJWVertex>::new(device),
            local_transform_arena: GPUArena::<LocalTransform>::new(device),
        }
    }
}

pub struct Renderer {
    allocations: Vec<u32>,
    vertex_arenas: VertexArenaCollection,
    global_transform_buffer: StaticGPUBuffer<GlobalTransform>,
    pub pipelines: PipelineCollection,
    passes: Vec<EngineRenderPass>,
    groups: Vec<RenderGroup>,
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
        }
    }
    pub fn add_pass(&mut self, label: String, categories: Vec<RenderCategory>) {
        self.passes.push(EngineRenderPass { label, categories });
    }

    /// Organize draw calls into Buffer -> DrawItem
    /// for each view that belongs to a buffer, find the allocation range within the buffer
    /// then find the final indices within that allocation range
    fn write_draw_list<V: ModelVertex>(
        &self,
        draw_map: &mut HashMap<usize, Vec<DrawItem>>,
        gt_map: &Vec<u16>,
    ) where
        DrawPacket: DrawListBuilder<V>,
        Self: VertexArenaSelector<V>,
    {
        let arena: &GPUArena<V> = self.get_arena();
        for group in self.groups.iter() {
            let instance_index = gt_map[group.instance_handle.global_id as usize];
            // for each allocation view
            for view in group.views.iter() {
                if !V::has_view_data(view) {
                    continue;
                }
                let buf_id = arena.chunk_id(&view.gpu_handle);
                // if the buffer has NOT already been visited, store the allocation range
                // in the alloc map, and an empty vec in the packet map
                let draw_list = draw_map.entry(buf_id).or_insert_with(|| vec![]);
                let (local_transform_range, _, _) = self
                    .vertex_arenas
                    .local_transform_arena
                    .resolve(&view.gpu_handle);

                DrawPacket::write_list(
                    view,
                    arena,
                    draw_list,
                    instance_index as u32,
                    local_transform_range.start,
                );
            }
        }
    }

    pub fn gen_draw_calls_new<'frame>(
        &'frame self,
        instance_manager: &'frame InstanceManager,
        queue: &wgpu::Queue,
    ) -> Option<DrawPacket> {
        let (gt_map, positions): (Vec<u16>, Vec<&'frame [GlobalTransform]>) =
            GlobalTransform::get_instance_data(instance_manager).unwrap();

        let mut packet = DrawPacket {
            pnu: HashMap::new(),
            pnujw: HashMap::new(),
        };

        // get the total length of the gt buffer
        let total_length: usize = positions.iter().fold(0, |acc, e| acc + e.len());
        if total_length == 0 {
            return None;
        }

        // populate the gt buffer with all global transform data
        if let Some(mut buffer_view) = queue.write_buffer_with(
            &self.global_transform_buffer,
            0,
            NonZero::new((total_length * size_of::<GlobalTransform>()) as u64).unwrap(),
        ) {
            let mut offset: usize = 0;
            for pos_slice in positions {
                buffer_view[offset..offset + pos_slice.len() * size_of::<GlobalTransform>()]
                    .copy_from_slice(bytemuck::cast_slice(pos_slice));
                offset += pos_slice.len() * size_of::<GlobalTransform>();
            }

            self.write_draw_list::<PNUVertex>(&mut packet.pnu, &gt_map);
            self.write_draw_list::<PNUJWVertex>(&mut packet.pnujw, &gt_map);
        }

        // for each instance

        Some(packet)
    }

    pub(super) fn add_render_group(
        &mut self,
        views: Vec<RenderView>,
        instance_handle: InstanceHandle,
    ) {
        self.groups.push(RenderGroup::new(instance_handle, views));
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
        job: LocalTransformUploadJob,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        self.vertex_arenas
            .local_transform_arena
            .upload(job, queue)?;
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
        draw_packet: DrawPacket,
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
                                        .buffer_from_chunk_id(*draw_entry.0)
                                        .slice(..),
                                );

                                for draw in draw_entry.1.iter() {
                                    render_pass
                                        .set_immediates(0, bytemuck::cast_slice(&[draw.lt_idx]));
                                    render_pass
                                        .draw(draw.primitives.clone(), draw.instances.clone());
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
                                        .buffer_from_chunk_id(*draw_entry.0)
                                        .slice(..),
                                );
                                for draw in draw_entry.1.iter() {
                                    render_pass
                                        .set_immediates(0, bytemuck::cast_slice(&[draw.lt_idx]));
                                    render_pass
                                        .draw(draw.primitives.clone(), draw.instances.clone());
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
