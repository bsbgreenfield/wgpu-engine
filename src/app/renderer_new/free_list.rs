use std::{error::Error, fmt::Display, marker::PhantomData};

use crate::{app::renderer_new::CHUNK_SIZE, util::types::ModelVertex};

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
enum FreeListAllocError {
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

    fn find_first(&self, size: u64) -> Result<usize, FreeListAllocError> {
        let mut node_idx = self.head;

        loop {
            let node = &self.nodes[node_idx];
            if node.block_size >= size {
                return Ok(node_idx);
            } else {
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

    fn alloc_first(&mut self, size: u64) -> Result<(), FreeListAllocError> {
        let node_idx = self.find_first(size)?;
        let remaining_node_space = self.nodes[node_idx].block_size - size;
        let new_node = FreeListNode::new(remaining_node_space, None);
        self.nodes.push(new_node);
        self.nodes[node_idx].next = Some(self.nodes.len() - 1);

        Ok(())
    }
}
