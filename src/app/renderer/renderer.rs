use std::num::NonZero;

use wgpu::RenderPass;

use crate::{
    app::{
        app_config::AppConfig,
        renderer::{
            DrawPacket, InstanceUploadJob, Instruction, RenderCategory, RenderConstant,
            RenderError, RenderUpdateDelta, RenderUpdateError, UploadMeshJob, VertexArenaError,
            VertexArenaSelector,
            gpu_allocator::{
                GPUAllocator, GPUInstanceAllocator, UploadIndexJob,
                instance_arena::InstanceArena,
                vertex_arena::{GPUArena, StaticGPUBuffer},
            },
            pipeline::PipelineCollection,
        },
    },
    util::types::{GlobalTransform, LocalTransform, PNUJWVertex, PNUVertex, VIndex},
    world::{
        camera::Camera,
        instance_manager::{InstanceHandle, RenderFrame},
        world::DrawSet,
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
}

struct VertexArenaCollection {
    index_arena: GPUArena<VIndex>,
    static_arena: GPUArena<PNUVertex>,
    skinned_arena: GPUArena<PNUJWVertex>,
}

impl VertexArenaCollection {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            index_arena: GPUArena::<VIndex>::new(device),
            static_arena: GPUArena::<PNUVertex>::new(device),
            skinned_arena: GPUArena::<PNUJWVertex>::new(device),
        }
    }
}

struct InstanceArenaCollection {
    lt_arena: InstanceArena<LocalTransform>,
    joint_arena: InstanceArena<LocalTransform>,
}

impl InstanceArenaCollection {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            lt_arena: InstanceArena::<LocalTransform>::new(device),
            joint_arena: InstanceArena::<LocalTransform>::new(device),
        }
    }
}

pub struct Renderer {
    allocations: Vec<u32>,
    vertex_arenas: VertexArenaCollection,
    instance_arenas: InstanceArenaCollection,
    global_transform_buffer: StaticGPUBuffer<GlobalTransform>,
    pub pipelines: PipelineCollection,
    passes: Vec<EngineRenderPass>,
}

impl Renderer {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            allocations: Vec::new(),
            vertex_arenas: VertexArenaCollection::new(&config.device),
            instance_arenas: InstanceArenaCollection::new(&config.device),
            global_transform_buffer: StaticGPUBuffer::<GlobalTransform>::new(&config.device),
            pipelines: PipelineCollection::new(config),
            passes: Vec::new(),
        }
    }
    pub fn add_pass(&mut self, label: String, categories: Vec<RenderCategory>) {
        self.passes.push(EngineRenderPass { label, categories });
    }

    pub(super) fn get_global_alloc_id(&mut self) -> u32 {
        self.allocations.push(self.allocations.len() as u32);
        (self.allocations.len() - 1) as u32
    }

    pub fn update(
        &mut self,
        constants: Vec<RenderConstant>,
        ops: Vec<Instruction>,
        queue: &wgpu::Queue,
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        self.interpret(constants, ops, queue)
    }

    pub fn prepare_frame(&mut self, render_frame: RenderFrame, queue: &wgpu::Queue) {
        'global_transforms: {
            let global_transforms = &render_frame.global_transforms;
            if global_transforms.is_empty() {
                break 'global_transforms;
            }
            let gt_size = global_transforms.iter().fold(0, |acc, e| acc + e.len());
            if let Some(mut buffer_view) = queue.write_buffer_with(
                &self.global_transform_buffer,
                0,
                NonZero::new(gt_size as u64).unwrap(),
            ) {
                let mut offset: usize = 0;
                for pos_slice in global_transforms {
                    buffer_view[offset..offset + pos_slice.len()]
                        .copy_from_slice(bytemuck::cast_slice(pos_slice));
                    offset += pos_slice.len();
                }
            }
        }
        'rigid_animations: {
            let animations = &render_frame.rigid_animation_data;
            if animations.is_empty() {
                break 'rigid_animations;
            }
            let buffer_ref = self.instance_arenas.lt_arena.get_first_buffer();
            for animation in animations {
                queue.write_buffer(
                    buffer_ref,
                    animation.buffer_offset.into(),
                    animation.transforms,
                );
            }
        }
    }

    pub(super) fn upload_indices<'frame>(
        &mut self,
        job: UploadIndexJob,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        self.vertex_arenas.index_arena.upload(job, queue)?;
        Ok(())
    }

    pub(super) fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
    ) -> Result<u32, VertexArenaError> {
        self.instance_arenas.lt_arena.upload(job, queue)
    }

    pub(super) fn upload_joint_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
    ) -> Result<u32, VertexArenaError> {
        self.instance_arenas.joint_arena.upload(job, queue)
    }

    pub(super) fn resolve_shared_lt_binding(
        &mut self,
        donor_handle: &InstanceHandle,
        new_handle: &InstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        self.instance_arenas
            .lt_arena
            .register_shared_binding(donor_handle, new_handle)
    }

    pub(super) fn resolve_shared_joint_binding(
        &mut self,
        donor_handle: &InstanceHandle,
        new_handle: &InstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        self.instance_arenas
            .joint_arena
            .register_shared_binding(donor_handle, new_handle)
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
                render_pass.set_bind_group(1, self.instance_arenas.lt_arena.bind_group(), &[]);
                render_pass.set_vertex_buffer(1, self.global_transform_buffer.slice(..));
                for render_category in pass.categories.iter() {
                    match render_category {
                        RenderCategory::OpaqueStatic => {
                            let pipeline = &self.pipelines.opaque_static;
                            render_pass.set_pipeline(&pipeline.pipeline);
                            for draw_entry in draw_packet.pnu.iter() {
                                let (vertex_alloc_range, v_buffer) =
                                    self.vertex_arenas.static_arena.resolve(draw_entry.0);

                                render_pass.set_vertex_buffer(0, v_buffer.slice(..));

                                for draw in draw_entry.1.iter() {
                                    render_pass
                                        .set_immediates(0, bytemuck::cast_slice(&[draw.lt_idx]));
                                    if let Some(indices) = &draw.indices {
                                        let (index_alloc_range, i_buffer) =
                                            self.vertex_arenas.index_arena.resolve(draw_entry.0);
                                        render_pass.set_index_buffer(
                                            i_buffer.slice(..),
                                            wgpu::IndexFormat::Uint16,
                                        );
                                        render_pass.draw_indexed(
                                            DrawSet::within(indices, &index_alloc_range),
                                            DrawSet::within(&draw.primitives, &vertex_alloc_range)
                                                .start
                                                as i32,
                                            draw.instances.clone(),
                                        );
                                    } else {
                                        render_pass.draw(
                                            DrawSet::within(&draw.primitives, &vertex_alloc_range),
                                            draw.instances.clone(),
                                        );
                                    }
                                }
                            }
                        }
                        RenderCategory::OpaqueSkinned => {
                            let pipeline = &self.pipelines.opaque_skinned;
                            render_pass.set_pipeline(&pipeline.pipeline);
                            for draw_entry in draw_packet.pnujw.iter() {
                                let (vertex_alloc_range, v_buffer) =
                                    self.vertex_arenas.skinned_arena.resolve(draw_entry.0);

                                render_pass.set_vertex_buffer(0, v_buffer.slice(..));

                                for draw in draw_entry.1.iter() {
                                    render_pass
                                        .set_immediates(0, bytemuck::cast_slice(&[draw.lt_idx]));
                                    if let Some(indices) = &draw.indices {
                                        let (index_alloc_range, i_buffer) =
                                            self.vertex_arenas.index_arena.resolve(draw_entry.0);
                                        render_pass.set_index_buffer(
                                            i_buffer.slice(..),
                                            wgpu::IndexFormat::Uint16,
                                        );
                                        render_pass.draw_indexed(
                                            DrawSet::within(indices, &index_alloc_range),
                                            DrawSet::within(&draw.primitives, &vertex_alloc_range)
                                                .start
                                                as i32,
                                            draw.instances.clone(),
                                        );
                                    } else {
                                        render_pass.draw(
                                            DrawSet::within(&draw.primitives, &vertex_alloc_range),
                                            draw.instances.clone(),
                                        );
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
}
