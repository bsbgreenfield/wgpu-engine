use crate::app::renderer_new::renderer_new::RenderCategory;

struct EnginePipeline {
    pub(super) pipeline: wgpu::RenderPipeline,
    category: RenderCategory,
    pub render_groups: Vec<crate::world::world::RenderGroup>,
}

impl EnginePipeline {}

pub(super) struct PipelineCollection {
    pub(super) opaque_skinned: EnginePipeline,
    pub(super) opaque_static: EnginePipeline,
}

impl PipelineCollection {
    pub(super) fn new() -> Self {
        use super::renderer_new::RenderCategory::*;
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
