use std::{collections::HashMap, ops::Range};

use super::GPUAllocationHandle;
use crate::app::renderer_new::renderer_new::{RenderCategory, RenderGroup};

struct DrawMap {
    map: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
}

pub(super) struct DrawItem {
    /// "local" refers to the allocation
    pub(super) local_mesh_id: u32,
    primitive_range: Range<u32>,
}
impl DrawItem {
    #[inline]
    pub(super) fn within(&self, range: &Range<u32>) -> Range<u32> {
        let start = range.start + self.primitive_range.start;
        start..(start + self.primitive_range.len() as u32)
    }
}

struct EnginePipeline {
    pub(super) pipeline: wgpu::RenderPipeline,
    category: RenderCategory,
    pub render_groups: Vec<RenderGroup>,
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
