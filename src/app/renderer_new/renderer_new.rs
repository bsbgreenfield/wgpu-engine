use std::{collections::hash_set::Iter, error::Error, fmt::Display, ops::Range};

use wgpu::RenderPass;

use crate::{
    app::{
        app_config::AppConfig,
        renderer_new::{
            GPUAllocator, Instruction, RenderUpdateDeltaNew, VMValue,
            pipeline::PipelineCollection,
            vertex_arena::{GPUArenaNew, LocalTransformUploadJob, VertexArenaError},
            vm::UploadMeshJob,
        },
    },
    util::types::{LocalTransform, ModelVertex, PNUJWVertex, PNUVertex},
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
}

impl VertexArenaCollection {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            static_arena: GPUArenaNew::<PNUVertex>::new(device),
            skinned_arena: GPUArenaNew::<PNUJWVertex>::new(device),
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

struct RenderModel {
    model_id: usize,
    meshes_ids: Vec<usize>,
}
struct RenderModelManager {
    render_models: Vec<RenderModel>,
}

pub struct RendererNew {
    allocations: Vec<u32>,
    local_transform_arena: GPUArenaNew<LocalTransform>,
    vertex_arenas: VertexArenaCollection,
    pipelines: PipelineCollection,
    passes: Vec<EngineRenderPass>,
}

impl RendererNew {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            allocations: Vec::new(),
            local_transform_arena: GPUArenaNew::<LocalTransform>::new(device),
            vertex_arenas: VertexArenaCollection::new(device),
            pipelines: PipelineCollection::new(),
            passes: Vec::new(),
        }
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
        self.local_transform_arena.upload(job, queue);
        Ok(())
    }

    pub fn render(&self, config: &AppConfig) -> Result<(), RenderError> {
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
            render_pass.set_bind_group(0, self.local_transform_arena.get_bind_group(), &[]);
            for render_category in &pass.categories {
                match render_category {
                    RenderCategory::OpaqueStatic => {
                        let pipeline = &self.pipelines.opaque_static;
                        render_pass.set_pipeline(&pipeline.pipeline);
                        // PROCESS VIEW
                    }
                    RenderCategory::OpaqueSkinned => {
                        render_pass.set_pipeline(&self.pipelines.opaque_skinned.pipeline);
                    }
                }
            }
        }
        Ok(())
    }
}
