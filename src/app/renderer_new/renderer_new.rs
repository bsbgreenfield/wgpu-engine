use std::{error::Error, fmt::Display, ops::Range};

use wgpu::RenderPass;

use crate::{
    app::{
        app_config::AppConfig,
        renderer_new::{
            GPUAllocator, Instruction, RenderUpdateDeltaNew, VMValue,
            pipeline::PipelineCollection,
            vertex_arena::{
                GPUArenaNew, LocalTransformUploadJob, StaticGPUBuffer, VertexArenaError,
            },
            vm::UploadMeshJob,
        },
    },
    util::types::{GlobalTransform, LocalTransform, ModelVertex, PNUJWVertex, PNUVertex},
    world::{
        components::ComponentData,
        instance_manager::{InstanceHandle, InstanceManager},
        world::{RenderGroup, RenderView},
    },
};

#[derive(Debug)]
pub enum RenderUpdateError {
    MeshUploadFailed(String),
    LocalTransformUpdateFailed,
}

impl From<VertexArenaError> for RenderUpdateError {
    fn from(value: VertexArenaError) -> Self {
        match value {
            VertexArenaError::DataTooLarge(size) => Self::MeshUploadFailed(format!(
                "upload failed because data of size {size} was too large"
            )),
            VertexArenaError::FreeListError(e) => {
                Self::MeshUploadFailed(format!("Upload failed due to allocation error {}", e))
            }
        }
    }
}

#[derive(Debug)]
pub enum RenderError {
    SurfaceError(wgpu::SurfaceError),
}

impl From<wgpu::SurfaceError> for RenderError {
    fn from(value: wgpu::SurfaceError) -> Self {
        Self::SurfaceError(value)
    }
}

impl Display for RenderUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MeshUploadFailed(desc) => desc.fmt(f),
            Self::LocalTransformUpdateFailed => {
                f.write_str("Local Transform data could not be uploaded")
            }
        }
    }
}

impl Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SurfaceError(e) => e.fmt(f),
        }
    }
}

impl Error for RenderUpdateError {}

pub(super) enum RenderCategory {
    OpaqueStatic,
    OpaqueSkinned,
}

struct DrawItem {
    instances: Range<u32>,
    primitives: Range<u32>,
    // TODO: indices
}

pub(super) struct EngineRenderPass {
    label: String,
    categories: Vec<RenderCategory>,
}

impl EngineRenderPass {
    fn create_pass<'frame>(
        &'frame self,
        encoder: &'frame mut wgpu::CommandEncoder,
        view: &'frame wgpu::TextureView,
    ) -> Result<RenderPass<'frame>, wgpu::SurfaceError> {
        // TODO match on render cat OR add generics to method call
        // TODO: customize render pass output
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(self.label.as_str()),
            depth_stencil_attachment: None, // TODO: depth stencil
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.6,
                        g: 0.3,
                        b: 0.3,
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
    static_arena: GPUArenaNew<PNUVertex>,
    skinned_arena: GPUArenaNew<PNUJWVertex>,
    local_transform_arena: GPUArenaNew<LocalTransform>,
}

impl VertexArenaCollection {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            static_arena: GPUArenaNew::<PNUVertex>::new(device),
            skinned_arena: GPUArenaNew::<PNUJWVertex>::new(device),
            local_transform_arena: GPUArenaNew::<LocalTransform>::new(device),
        }
    }
}

trait VertexArenaSelector<V: ModelVertex> {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<V>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError>;
}

impl VertexArenaSelector<PNUJWVertex> for RendererNew {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<PNUJWVertex>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        let handle = self.vertex_arenas.skinned_arena.upload(mesh_job, queue)?;
        Ok(())
    }
}

impl VertexArenaSelector<PNUVertex> for RendererNew {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<PNUVertex>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        let handle = self.vertex_arenas.static_arena.upload(mesh_job, queue)?;
        // TODO handle?
        Ok(())
    }
}

pub struct RendererNew {
    allocations: Vec<u32>,
    vertex_arenas: VertexArenaCollection,
    global_transform_buffer: StaticGPUBuffer<GlobalTransform>,
    pipelines: PipelineCollection,
    passes: Vec<EngineRenderPass>,
    groups: Vec<RenderGroup>,
}

impl RendererNew {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            allocations: Vec::new(),
            vertex_arenas: VertexArenaCollection::new(device),
            global_transform_buffer: StaticGPUBuffer::<GlobalTransform>::new(device),
            pipelines: PipelineCollection::new(),
            passes: Vec::new(),
            groups: Vec::new(),
        }
    }

    pub fn gen_draw_calls<'frame>(
        &'frame self,
        instance_manager: &'frame InstanceManager,
    ) -> Vec<DrawItem> {
        let (gt_map, positions): (Vec<u16>, Vec<&'frame GlobalTransform>) =
            GlobalTransform::get_instance_data(instance_manager).unwrap();
        self.
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
    ) -> Result<Vec<RenderUpdateDeltaNew>, RenderUpdateError> {
        self.interpret(constants, ops, queue)
    }

    pub(super) fn upload_mesh_data<'frame, V: ModelVertex>(
        &mut self,
        mesh_job: UploadMeshJob<'frame, V>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError>
    where
        Self: VertexArenaSelector<V>,
    {
        self.upload_mesh(mesh_job, queue)
    }

    pub(super) fn upload_local_transform_data<'frame>(
        &mut self,
        job: LocalTransformUploadJob,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        self.vertex_arenas.local_transform_arena.upload(job, queue);
        Ok(())
    }

    pub fn render(
        &self,
        config: &AppConfig,
        instance_manager: &InstanceManager,
    ) -> Result<(), RenderError> {
        let draws = self.gen_draw_calls(instance_manager);

        for pass in &self.passes {
            let output = config.surface.get_current_texture()?;
            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder =
                config
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some(format!("Render Encoder for {}", pass.label).as_str()),
                    });
            let mut render_pass = pass.create_pass(&mut encoder, &view)?;
            render_pass.set_bind_group(
                0,
                self.vertex_arenas.local_transform_arena.get_bind_group(),
                &[],
            );
            //
            //            for render_group in self.groups.iter() {
            //                for render_view in render_group.views.iter() {
            //                    let (lt_index_range, _, lt_bind_group) = self
            //                        .vertex_arenas
            //                        .local_transform_arena
            //                        .resolve(&render_view.gpu_handle);
            //                    for render_category in &pass.categories {
            //                        match render_category {
            //                            RenderCategory::OpaqueStatic => {
            //                                let pipeline = &self.pipelines.opaque_static;
            //                                render_pass.set_pipeline(&pipeline.pipeline);
            //                                render_pass.set_bind_group(1, lt_bind_group, &[]);
            //                                let (alloc_range, buffer, _) = self
            //                                    .vertex_arenas
            //                                    .static_arena
            //                                    .resolve(&render_view.gpu_handle);
            //                                render_pass.set_vertex_buffer(0, buffer.slice(..));
            //                                let mesh_ids = &render_view.pnu_draws.mesh_ids;
            //                                let prim_ranges = &render_view.pnu_draws.primtitive_ranges;
            //                                for i in 0..render_view.pnu_draws.mesh_ids.len() {
            //                                    render_pass.set_immediates(
            //                                        0,
            //                                        bytemuck::cast_slice(&[lt_index_range.start + mesh_ids[i]]),
            //                                    );
            //                                    render_pass
            //                                        .draw(DrawSet::within(&prim_ranges[i], &alloc_range), 0..1);
            //                                }
            //                            }
            //                            _ => todo!(),
            //                        }
            //                    }
            //                }
            //            }
            //            for render_category in &pass.categories {
            //                match render_category {
            //                    RenderCategory::OpaqueStatic => {
            //                        let pipeline = &self.pipelines.opaque_static;
            //                        render_pass.set_pipeline(&pipeline.pipeline);
            //                        // PROCESS VIEW
            //                    }
            //                    RenderCategory::OpaqueSkinned => {
            //                        render_pass.set_pipeline(&self.pipelines.opaque_skinned.pipeline);
            //                    }
            //                }
            //            }
            //        }
        }
        Ok(())
    }
}
