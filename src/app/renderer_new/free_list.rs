use std::{error::Error, fmt::Display, marker::PhantomData, ops::Range};

use crate::{
    app::renderer_new::{CHUNK_SIZE, vertex_arena::VertexArenaError},
    util::types::ModelVertex,
};

pub(super) struct FreeListAllocator<V: ModelVertex> {
    nodes: Vec<FreeListNode>,
    chunk_size: u64,
    used: u64,
    head: usize,
    _v: PhantomData<V>,
}

struct FreeListNode {
    block_size: u64,
    next: Option<usize>,
}

impl<'chunk> FreeListNode {
    fn new(block_size: u64, next: Option<usize>) -> Self {
        Self { block_size, next }
    }
}

#[derive(Debug)]
pub(super) enum FreeListAllocError {
    NoRoomLeft(u64, u64),
}

impl Error for FreeListAllocError {}
impl Display for FreeListAllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRoomLeft(size, used) => f.write_str(
                format!(
                    "Not enough room to fit data of size {}. Available: {}",
                    size, used,
                )
                .as_str(),
            ),
        }
    }
}

impl<V: ModelVertex> FreeListAllocator<V> {
    pub(super) fn new() -> Self {
        Self {
            nodes: vec![FreeListNode::new(CHUNK_SIZE, None)],
            chunk_size: CHUNK_SIZE,
            used: 0,
            head: 0,
            _v: PhantomData,
        }
    }

    fn find_first(&self, size: u64) -> Result<(usize, usize), FreeListAllocError> {
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

    pub(super) fn alloc_first(&mut self, size: u64) -> Result<u64, FreeListAllocError> {
        // TODO: account for alignemnt and padding
        let (offset, node_idx) = self.find_first(size)?;
        let node = &mut self.nodes[node_idx];
        let remaining_node_space = node.block_size - size;
        node.block_size -= remaining_node_space;
        let new_node = FreeListNode::new(remaining_node_space, None);
        self.nodes.push(new_node);
        self.nodes[node_idx].next = Some(self.nodes.len() - 1);

        Ok(offset as u64)
    }
}
