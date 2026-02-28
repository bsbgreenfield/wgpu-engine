use crate::{
    app::renderer_new::vertex_arena::VertexArenaNew,
    util::types::{PNUJWVertex, PNUVertex},
};

enum RenderCategory {
    OpaqueStatic,
    OpaqueSkinned,
}

pub(super) struct EngineRenderPass {
    label: String,
    categories: Vec<RenderCategory>,
}

impl EngineRenderPass {
    fn create_pass(&self) {
        todo!()
    }

    fn new(label: &str, categories: Vec<RenderCategory>) -> Self {
        Self {
            label: label.to_owned(),
            categories,
        }
    }
}

struct EnginePipeline {
    pipeline: wgpu::RenderPipeline,
    category: RenderCategory,
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
    static_arena: VertexArenaNew<PNUVertex>,
    skinned_arena: VertexArenaNew<PNUJWVertex>,
}

impl VertexArenaCollection {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            static_arena: VertexArenaNew::new(device),
            skinned_arena: VertexArenaNew::new(device),
        }
    }
}

pub struct RendererNew {
    vertex_arenas: VertexArenaCollection,
    pipelines: PipelineCollection,
    passes: Vec<EngineRenderPass>,
}

impl RendererNew {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            vertex_arenas: VertexArenaCollection::new(device),
            pipelines: PipelineCollection::new(),
            passes: Vec::new(),
        }
    }
}
