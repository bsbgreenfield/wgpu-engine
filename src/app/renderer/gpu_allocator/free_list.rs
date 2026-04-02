use std::ops::Range;

use crate::app::renderer::gpu_allocator::{CHUNK_SIZE, FreeListAllocError};

pub(super) struct FreeListAllocator {
    nodes: Vec<FreeListNode>,
    chunk_size: u32,
    used: u32,
    head: usize,
}

struct FreeListNode {
    block_size: u32,
    offset: u32,
    next: Option<usize>,
}

impl<'chunk> FreeListNode {
    fn new(block_size: u32, next: Option<usize>, offset: u32) -> Self {
        Self {
            block_size,
            next,
            offset,
        }
    }
}

impl FreeListAllocator {
    #[inline]
    pub(super) fn resolve(&self, node_id: usize) -> Range<u32> {
        let node = &self.nodes[node_id];
        node.offset..node.offset + node.block_size
    }

    #[inline]
    pub(super) fn offset_of(&self, node_id: usize) -> u64 {
        self.nodes[node_id].offset as u64
    }
    pub(super) fn new() -> Self {
        Self {
            nodes: vec![FreeListNode::new(CHUNK_SIZE, None, 0)],
            chunk_size: CHUNK_SIZE,
            used: 0,
            head: 0,
        }
    }

    fn find_first(&self, size: u32) -> Result<(usize, usize), FreeListAllocError> {
        let mut offset = 0;
        let mut node_idx = self.head;

        loop {
            let node = &self.nodes[node_idx];
            if node.block_size >= size {
                return Ok((offset, node_idx));
            } else {
                offset += node.block_size as usize;
                if let Some(next_idx) = node.next {
                    node_idx = next_idx;
                    continue;
                }
                break;
            }
        }

        Err(FreeListAllocError::NoRoomLeft(
            size,
            self.chunk_size - self.used,
        ))
    }

    pub(super) fn alloc_first(&mut self, size: u32) -> Result<usize, FreeListAllocError> {
        // TODO: account for alignemnt and padding
        let (offset, node_idx) = self.find_first(size)?;
        let node = &mut self.nodes[node_idx];
        let remaining_node_space = node.block_size - size;
        node.block_size -= remaining_node_space;
        let new_node = FreeListNode::new(remaining_node_space, None, offset as u32 + size);
        self.nodes.push(new_node);
        self.nodes[node_idx].next = Some(self.nodes.len() - 1);

        Ok(node_idx)
    }
}
