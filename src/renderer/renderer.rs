use core::hash;
use std::ops::Deref;

use wgpu::{CurrentSurfaceTexture, RenderPass};

use crate::{
    app::app_config::AppConfig,
    renderer::{
        DrawPacket, GPUAllocationHandle, GPUInstanceHandle, InstanceUploadJob, Instruction,
        PrototypeHandle, RenderCategory, RenderConstant, RenderError, RenderUpdateDelta,
        RenderUpdateError, UploadMeshJob, VertexArenaError, VertexArenaSelector,
        bind_groups::BindGroupCollection,
        gpu_allocator::{GPUAllocator, GPUUploadResult, UploadIndexJob, gpu_arena::GPUArena},
        pipeline::PipelineCollection,
    },
    util::types::{
        InstanceRecordData, InverseBindMatrix, JointTransform, LocalTransform, PNUJWVertex,
        PNUVertex, VIndex,
    },
    world::{RenderKey, camera::Camera, instance_manager::RenderFrame, world::DrawSet},
};

struct EngineRenderPass {
    label: String,
    categories: Vec<RenderCategory>,
}

impl EngineRenderPass {
    fn create_pass<'frame>(
        label: &'frame str,
        encoder: &'frame mut wgpu::CommandEncoder,
        view: &'frame wgpu::TextureView,
    ) -> Result<RenderPass<'frame>, wgpu::CreateSurfaceError> {
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
    fn new() -> Self {
        Self {
            index_arena: GPUArena::<VIndex>::new(),
            static_arena: GPUArena::<PNUVertex>::new(),
            skinned_arena: GPUArena::<PNUJWVertex>::new(),
        }
    }

    fn unload(&mut self, alloc_handle: GPUAllocationHandle) -> Result<(), VertexArenaError> {
        self.index_arena.remove(&alloc_handle)?;
        self.static_arena.remove(&alloc_handle)?;
        self.skinned_arena.remove(&alloc_handle)?;
        Ok(())
    }
}

impl RenderKey for GPUInstanceHandle {
    fn as_key(&self) -> u64 {
        let i = self.instance_id as u64;
        let p = (self.prototype.0 as u64) << 32;
        i | p
    }

    fn from_key(key: u64) -> Self {
        let instance = (key & 0xFFFF_FFFF) as u32;
        let p = ((key >> 32) & 0xFFFF_FFFF) as u32;

        let prototype = PrototypeHandle(p);

        Self {
            prototype,
            instance_id: instance,
        }
    }
}

pub(crate) struct Renderer {
    allocations: Vec<u32>,
    vertex_arenas: VertexArenaCollection,
    pub(super) bind_groups: BindGroupCollection,
    pipelines: Option<PipelineCollection>,
    passes: Vec<EngineRenderPass>,
}

impl Renderer {
    #[cfg(test)]
    pub(crate) fn get_prototype_count(&self) -> usize {
        self.bind_groups.get_prototype_count()
    }
    #[cfg(test)]
    pub(crate) fn get_prototype_ref_count(&self, handle: &PrototypeHandle) -> Option<usize> {
        self.bind_groups.get_prototype_ref_count(handle)
    }

    #[allow(unused)]
    #[cfg(test)]
    pub(crate) fn get_lt_buffer(&self) -> &wgpu::Buffer {
        self.bind_groups.get_lt_buffer()
    }
    #[cfg(test)]
    pub(crate) fn get_joint_buffers(&self) -> (&wgpu::Buffer, &wgpu::Buffer) {
        self.bind_groups.get_joint_buffer()
    }

    #[cfg(test)]
    pub(crate) fn get_instance_record_buffer(&self) -> &wgpu::Buffer {
        self.bind_groups.instance_data.get_first_record_buffer()
    }

    pub(crate) fn new() -> Self {
        Self {
            allocations: Vec::new(),
            vertex_arenas: VertexArenaCollection::new(),
            bind_groups: BindGroupCollection::new(),
            pipelines: None,
            passes: Vec::new(),
        }
    }

    pub(crate) fn init(&mut self, config: &AppConfig) {
        let pipeline_collection = PipelineCollection::new(config);
        self.pipelines = Some(pipeline_collection);
    }

    pub(crate) fn add_pass(&mut self, label: String, categories: Vec<RenderCategory>) {
        self.passes.push(EngineRenderPass { label, categories });
    }

    pub(super) fn get_global_alloc_id(&mut self) -> u32 {
        self.allocations.push(self.allocations.len() as u32);
        (self.allocations.len() - 1) as u32
    }

    pub(super) fn get_gpu_instance_handle(
        &mut self,
        prototype: &PrototypeHandle,
    ) -> GPUInstanceHandle {
        self.bind_groups.gen_gpu_instance_handle(prototype)
    }

    pub(super) fn add_prototype(&mut self, prototype: PrototypeHandle) {
        self.bind_groups.add_prototype(prototype);
    }
    pub(super) fn add_prototype_instance(&mut self, prototype: &PrototypeHandle) {
        self.bind_groups.add_prototype_instance(prototype);
    }
    pub(crate) fn update(
        &mut self,
        constants: Vec<RenderConstant>,
        ops: Vec<Instruction>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        self.interpret(constants, ops, queue, device)
    }

    pub(crate) fn prepare_frame(&mut self, render_frame: RenderFrame, queue: &wgpu::Queue) {
        // offsets
        if !render_frame.indirection_list.is_empty() {
            self.bind_groups
                .instance_data
                .upload_instance_offsets(render_frame.indirection_list, queue);
        }

        'global_transforms: {
            if render_frame.global_transforms.is_empty() {
                break 'global_transforms;
            }
            self.bind_groups
                .instance_data
                .write_gt_data(bytemuck::cast_slice(render_frame.global_transforms), queue);
        }
        'rigid_animations: {
            let animations = &render_frame.rigid_animation_data;
            if animations.is_empty() {
                break 'rigid_animations;
            }
            for animation in animations {
                self.bind_groups.local_transforms.write_lt_anim_data(
                    &animation.gpu_handle,
                    animation.transforms,
                    queue,
                );
            }
        }
        'skinned_animations: {
            let animations = &render_frame.joint_animation_data;
            if animations.is_empty() {
                break 'skinned_animations;
            }
            for animation in animations {
                self.bind_groups.skinning.write_joint_anim_data(
                    &animation.gpu_handle,
                    animation.transforms,
                    queue,
                );
            }
        }
    }

    pub(super) fn despawn_instance(&mut self, handle: &GPUInstanceHandle) {
        self.bind_groups.despawn(handle);
    }

    pub(super) fn unload_asset(
        &mut self,
        alloc_handle: GPUAllocationHandle,
    ) -> Result<(), VertexArenaError> {
        self.vertex_arenas.unload(alloc_handle)
    }

    pub(super) fn upload_indices<'frame>(
        &mut self,
        job: UploadIndexJob,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError> {
        self.vertex_arenas.index_arena.upload(job, queue, device)?;
        Ok(())
    }

    pub(super) fn upload_instance_record<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, InstanceRecordData>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        Ok(self
            .bind_groups
            .instance_data
            .upload_instance_record(job, queue, device)?)
    }

    pub(super) fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        self.bind_groups.upload_local_transforms(job, queue, device)
    }

    pub(super) fn upload_skin_data<'frame>(
        &mut self,
        joint_job: InstanceUploadJob<'frame, JointTransform>,
        ibm_job: InstanceUploadJob<'frame, InverseBindMatrix>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        self.bind_groups
            .upload_skin_data(joint_job, ibm_job, queue, device)
    }

    pub(crate) fn render_blank(&self, config: &AppConfig) -> Result<(), RenderError> {
        let texture = match config.surface.as_ref().unwrap().get_current_texture() {
            CurrentSurfaceTexture::Success(texture) => texture,
            _ => return Err(RenderError::BadSurfaceTexture),
        };
        let view = texture
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
        config.queue.present(texture);
        Ok(())
    }

    pub(crate) fn render(
        &self,
        config: &AppConfig,
        camera: &Camera,
        draw_packet: &DrawPacket,
    ) -> Result<(), RenderError> {
        let pipeline_collection = self.pipelines.as_ref().unwrap();
        for pass in &self.passes {
            let texture = match config.surface.as_ref().unwrap().get_current_texture() {
                CurrentSurfaceTexture::Success(texture) => texture,
                _ => return Err(RenderError::BadSurfaceTexture),
            };
            let view = texture
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

                // camera bind group
                render_pass.set_bind_group(0, camera.get_bind_group(), &[]);
                // local transform bind group
                render_pass.set_bind_group(
                    1,
                    self.bind_groups.local_transforms.get_first_bg(),
                    &[],
                );
                render_pass.set_bind_group(2, self.bind_groups.instance_data.get_first_bg(), &[]);
                for render_category in pass.categories.iter() {
                    match render_category {
                        RenderCategory::OpaqueStatic => {
                            let pipeline = &pipeline_collection.opaque_static;
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
                                            DrawSet::within(indices, &index_alloc_range).into(),
                                            DrawSet::within(&draw.primitives, &vertex_alloc_range)
                                                .start
                                                as i32,
                                            draw.instances.clone().into(),
                                        );
                                    } else {
                                        render_pass.draw(
                                            DrawSet::within(&draw.primitives, &vertex_alloc_range)
                                                .into(),
                                            draw.instances.clone().into(),
                                        );
                                    }
                                }
                            }
                        }
                        RenderCategory::OpaqueSkinned => {
                            if draw_packet.pnujw.is_empty() {
                                continue;
                            }
                            let pipeline = &pipeline_collection.opaque_skinned;
                            render_pass.set_pipeline(&pipeline.pipeline);
                            render_pass.set_bind_group(
                                3,
                                self.bind_groups.skinning.get_first_bg(),
                                &[],
                            );
                            for draw_entry in draw_packet.pnujw.iter() {
                                let (vertex_alloc_range, v_buffer) =
                                    self.vertex_arenas.skinned_arena.resolve(draw_entry.0);

                                render_pass.set_vertex_buffer(0, v_buffer.slice(..));

                                for draw in draw_entry.1.iter() {
                                    render_pass.set_immediates(
                                        0,
                                        bytemuck::cast_slice(&[
                                            draw.lt_idx,
                                            draw.joint_offset.unwrap(),
                                        ]),
                                    );
                                    if let Some(indices) = &draw.indices {
                                        let (index_alloc_range, i_buffer) =
                                            self.vertex_arenas.index_arena.resolve(draw_entry.0);
                                        render_pass.set_index_buffer(
                                            i_buffer.slice(..),
                                            wgpu::IndexFormat::Uint16,
                                        );
                                        render_pass.draw_indexed(
                                            DrawSet::within(indices, &index_alloc_range).into(),
                                            DrawSet::within(&draw.primitives, &vertex_alloc_range)
                                                .start
                                                as i32,
                                            draw.instances.clone().into(),
                                        );
                                    } else {
                                        render_pass.draw(
                                            DrawSet::within(&draw.primitives, &vertex_alloc_range)
                                                .into(),
                                            draw.instances.clone().into(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            config.queue.submit(std::iter::once(encoder.finish()));
            config.queue.present(texture);
        }
        Ok(())
    }
}
impl VertexArenaSelector<PNUJWVertex> for Renderer {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<PNUJWVertex>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError> {
        let _handle = self
            .vertex_arenas
            .skinned_arena
            .upload(mesh_job, queue, device)?;
        Ok(())
    }
}

impl VertexArenaSelector<PNUVertex> for Renderer {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<PNUVertex>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError> {
        let _handle = self
            .vertex_arenas
            .static_arena
            .upload(mesh_job, queue, device)?;
        // TODO handle?
        Ok(())
    }
}
