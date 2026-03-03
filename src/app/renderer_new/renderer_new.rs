use std::{collections::HashMap, error::Error, fmt::Display, iter::FlatMap, ops::Range, rc::Rc};

use wgpu::RenderPass;

use crate::{
    app::{
        app_config::AppConfig,
        render::{Instruction, VMValue, renderer::RenderUpdateDelta},
        renderer_new::vertex_arena::{
            AllocationHandle, GPUArenaNew, UploadMeshJob, VertexArenaError,
        },
    },
    util::types::{LocalTransform, ModelVertex, PNUJWVertex, PNUVertex},
};

#[derive(Debug)]
pub enum RenderUpdateError {
    MeshUploadFailed(String),
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

enum RenderCategory {
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

struct DrawMap {
    map: HashMap<AllocationHandle, Vec<DrawItem>>,
}

struct DrawItem {
    primitive_range: Range<u32>,
}
impl DrawItem {
    #[inline]
    fn within(&self, range: &Range<u32>) -> Range<u32> {
        let start = range.start + self.primitive_range.start;
        start..(start + self.primitive_range.len() as u32)
    }
}

struct EnginePipeline {
    pipeline: wgpu::RenderPipeline,
    category: RenderCategory,
    draw_map: HashMap<AllocationHandle, Vec<DrawItem>>,
}

struct PipelineCollection {
    opaque_skinned: EnginePipeline,
    opaque_static: EnginePipeline,
}

impl PipelineCollection {
    fn new() -> Self {
        use RenderCategory::*;
        Self {
            opaque_skinned: Self::create_pipeline(OpaqueStatic),
            opaque_static: Self::create_pipeline(OpaqueSkinned),
        }
    }

    fn create_pipeline(cat: RenderCategory) -> EnginePipeline {
        match cat {
            RenderCategory::OpaqueStatic => todo!(),
            RenderCategory::OpaqueSkinned => todo!(),
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

trait ArenaSelector<V: ModelVertex> {
    fn upload(
        &mut self,
        mesh_job: UploadMeshJob<V>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError>;
}

impl ArenaSelector<PNUJWVertex> for RendererNew {
    fn upload(
        &mut self,
        mesh_job: UploadMeshJob<PNUJWVertex>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        let mut draws = Vec::<DrawItem>::new();
        for primitive_range in mesh_job.primitive_ranges {
            draws.push(DrawItem { primitive_range });
        }
        let handle = self.vertex_arenas.skinned_arena.upload_mesh(
            mesh_job.verts,
            mesh_job.global_alloc_id,
            queue,
        )?;

        self.pipelines.opaque_skinned.draw_map.insert(handle, draws);
        Ok(())
    }
}

impl ArenaSelector<PNUVertex> for RendererNew {
    fn upload(
        &mut self,
        mesh_job: UploadMeshJob<PNUVertex>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        let mut draws = Vec::<DrawItem>::new();
        for primitive_range in mesh_job.primitive_ranges {
            draws.push(DrawItem { primitive_range });
        }
        let handle = self.vertex_arenas.static_arena.upload_mesh(
            mesh_job.verts,
            mesh_job.global_alloc_id,
            queue,
        )?;

        self.pipelines.opaque_static.draw_map.insert(handle, draws);
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
    local_transform_arena: GPUArenaNew<LocalTransform>,
    vertex_arenas: VertexArenaCollection,
    pipelines: PipelineCollection,
    passes: Vec<EngineRenderPass>,
}

impl RendererNew {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            local_transform_arena: GPUArenaNew::<LocalTransform>::new(device),
            vertex_arenas: VertexArenaCollection::new(device),
            pipelines: PipelineCollection::new(),
            passes: Vec::new(),
        }
    }

    pub fn update(
        &mut self,
        constants: Vec<VMValue>,
        ops: Vec<Instruction>,
        queue: &wgpu::Queue,
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        self.interpret(constants, ops, queue)
    }

    pub(super) fn upload_mesh_data<'frame, V: ModelVertex>(
        &mut self,
        mesh_job: UploadMeshJob<'frame, V>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError>
    where
        Self: ArenaSelector<V>,
    {
        self.upload(mesh_job, queue)
    }

    pub(super) fn upload_local_transform_data<'frame>(&mut self, queue: &wgpu::Queue) {
        todo!()
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
            for render_category in &pass.categories {
                match render_category {
                    RenderCategory::OpaqueStatic => {
                        let ref pipeline = self.pipelines.opaque_static;
                        render_pass.set_pipeline(&pipeline.pipeline);
                        // iterate over per asset allocations for this pipeline
                        for (allocation_handle, draws) in pipeline.draw_map.iter() {
                            let (alloc_range, vertex_buf) =
                                self.vertex_arenas.static_arena.resolve(allocation_handle);
                            render_pass.set_vertex_buffer(0, vertex_buf.slice(..));
                            for draw in draws {
                                render_pass.draw(draw.within(&alloc_range), 0..1);
                            }
                        }
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
