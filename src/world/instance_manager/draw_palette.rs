use std::ops::Range;

use crate::app::renderer::{DrawItem, GPUAllocationHandle};

#[derive(Debug)]
pub(super) enum DrawData {
    Skinned(PipelineDrawData),
    Static(PipelineDrawData),
}

impl DrawData {
    pub(super) fn as_draw_item(&self, instance_idx: u32) -> DrawItem {
        match self {
            Self::Skinned(data) | Self::Static(data) => {
                return DrawItem {
                    lt_idx: data.lt_idx,
                    joint_offset: data.joint_offset,
                    instances: instance_idx..instance_idx + 1,
                    primitives: data.primitives.clone(),
                    indices: data.indices.clone(),
                };
            }
        }
    }
}
#[derive(Debug)]
pub(super) struct PipelineDrawData {
    pub(super) alloc_handle: GPUAllocationHandle,
    pub(super) lt_idx: u32,
    pub(super) joint_offset: Option<u32>,
    pub(super) primitives: Range<u32>,
    pub(super) indices: Option<Range<u32>>,
}

pub(super) struct DrawPalette {
    pub(super) data: Vec<DrawData>,
    pub(super) chunks: Vec<PaletteChunk>,
}

#[derive(Default)]
pub(super) struct PaletteChunk {
    offset: usize,
    len: usize,
}

impl PaletteChunk {
    pub(super) fn populate_data(&mut self, offset: usize, len: usize) {
        self.offset = offset;
        self.len = len;
    }
}

impl DrawPalette {
    pub fn new() -> Self {
        DrawPalette {
            chunks: Vec::new(),
            data: Vec::new(),
        }
    }

    pub fn iter<'a>(&'a self) -> DrawDataIter<'a> {
        DrawDataIter {
            pointer: 0,
            palette: self,
        }
    }
}

pub(super) struct DrawDataIter<'a> {
    palette: &'a DrawPalette,
    pointer: usize,
}

impl<'a> Iterator for DrawDataIter<'a> {
    type Item = &'a [DrawData];

    fn next(&mut self) -> Option<Self::Item> {
        if self.pointer < self.palette.chunks.len() {
            let chunk = &self.palette.chunks[self.pointer];
            self.pointer += 1;
            Some(&self.palette.data[chunk.offset..chunk.offset + chunk.len])
        } else {
            None
        }
    }
}
