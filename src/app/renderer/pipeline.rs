use crate::app::renderer::renderer::RenderCategory;

pub struct EnginePipeline {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub render_groups: Vec<crate::world::world::RenderGroup>,
}

impl EnginePipeline {}

pub struct PipelineCollection {
    pub opaque_skinned: EnginePipeline,
    pub opaque_static: EnginePipeline,
}

impl PipelineCollection {
    pub(super) fn new() -> Self {
        use super::renderer::RenderCategory::*;
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
