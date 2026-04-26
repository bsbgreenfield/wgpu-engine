use std::ops::Range;

use crate::app::renderer::gpu_allocator::{CHUNK_SIZE, FreeListAllocError};

pub(super) struct FreeListAllocator {
    nodes: Vec<FreeListNode>,
    chunk_size: u32,
    used: u32,
    head: usize,
    minimum_node_size: usize,
}

struct FreeListNode {
    block_size: u32,
    occupied: bool,
    offset: u32,
    next: Option<usize>,
}

impl<'chunk> FreeListNode {
    fn new(block_size: u32, next: Option<usize>, offset: u32) -> Self {
        Self {
            block_size,
            occupied: false,
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
    pub(super) fn new(minimum_node_size: usize) -> Self {
        Self {
            nodes: vec![FreeListNode::new(CHUNK_SIZE, None, 0)],
            chunk_size: CHUNK_SIZE,
            used: 0,
            head: 0,
            minimum_node_size,
        }
    }

    fn find_first(&self, size: u32) -> Result<(usize, usize), FreeListAllocError> {
        let mut offset = 0;
        let mut node_idx = self.head;

        loop {
            let node = &self.nodes[node_idx];

            // if the node is available and large enough, use this node
            if !node.occupied && node.block_size >= size {
                return Ok((offset, node_idx));
            }
            // otherwise, increment offset and move to the next node
            // if there is no next node, we are out of space
            else {
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

    #[cfg(test)]
    fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn alloc_first(&mut self, size: u32) -> Result<usize, FreeListAllocError> {
        // TODO: account for alignemnt and padding
        let (offset, node_idx) = self.find_first(size)?;
        let node = &mut self.nodes[node_idx];
        let remaining_node_space = node.block_size - size; // space remaining in this node after allocation
        node.occupied = true;

        // if this allocation has created a new empty node directly after it with at least 2 kb, mark this empty space
        // as a new node, add it to the list, and set it as .next for the selected node
        if remaining_node_space > self.minimum_node_size as u32 {
            node.block_size -= remaining_node_space;
            let new_node = FreeListNode::new(
                remaining_node_space,
                self.nodes[node_idx].next,
                offset as u32 + size,
            );
            self.nodes.push(new_node);
            self.nodes[node_idx].next = Some(self.nodes.len() - 1);
        }
        // if there is not enough space for a new node, do nothing

        Ok(node_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOC: u32 = 1024 * 64; // 64 KB

    #[test]
    fn first_alloc_offset_is_zero() {
        let mut alloc = FreeListAllocator::new(2048);
        let id = alloc.alloc_first(ALLOC).unwrap();
        assert_eq!(alloc.offset_of(id), 0);
    }

    #[test]
    fn resolve_returns_correct_range() {
        let mut alloc = FreeListAllocator::new(2048);
        let id = alloc.alloc_first(ALLOC).unwrap();
        let range = alloc.resolve(id);
        assert_eq!(range.start, 0);
        assert_eq!(range.end, ALLOC);
    }

    // ── sequential allocation offsets ─────────────────────────────────────────

    #[test]
    fn second_alloc_starts_after_first() {
        let mut alloc = FreeListAllocator::new(2048);
        let a = alloc.alloc_first(ALLOC).unwrap();
        let b = alloc.alloc_first(ALLOC).unwrap();
        assert_ne!(
            alloc.offset_of(a),
            alloc.offset_of(b),
            "two distinct allocations must not share the same buffer offset"
        );
        assert_eq!(
            alloc.offset_of(b),
            ALLOC as u64,
            "second allocation should start immediately after the first"
        );
    }

    #[test]
    fn three_allocs_are_contiguous() {
        let mut alloc = FreeListAllocator::new(2048);
        let a = alloc.alloc_first(ALLOC).unwrap();
        let b = alloc.alloc_first(ALLOC).unwrap();
        let c = alloc.alloc_first(ALLOC).unwrap();
        assert_eq!(alloc.offset_of(a), 0);
        assert_eq!(alloc.offset_of(b), ALLOC as u64);
        assert_eq!(alloc.offset_of(c), (ALLOC * 2) as u64);
    }

    /// Allocations of mixed sizes must also land contiguously with no gaps or
    /// overlaps between them.
    #[test]
    fn allocated_ranges_do_not_overlap() {
        let mut alloc = FreeListAllocator::new(2048);
        let sizes = [ALLOC, ALLOC * 2, ALLOC];
        let ids: Vec<usize> = sizes
            .iter()
            .map(|&s| alloc.alloc_first(s).unwrap())
            .collect();
        let ranges: Vec<_> = ids.iter().map(|&id| alloc.resolve(id)).collect();

        for i in 0..ranges.len() {
            for j in (i + 1)..ranges.len() {
                let (a, b) = (&ranges[i], &ranges[j]);
                assert!(
                    a.end <= b.start || b.end <= a.start,
                    "ranges[{i}] ({a:?}) and ranges[{j}] ({b:?}) overlap"
                );
            }
        }
    }

    // ── remainder-node threshold ───────────────────────────────────────────────

    /// A remainder larger than 2 KB must produce a new free node.
    #[test]
    fn large_remainder_creates_new_node() {
        let mut alloc = FreeListAllocator::new(2048);
        let before = alloc.node_count();
        alloc.alloc_first(ALLOC).unwrap();
        assert_eq!(
            alloc.node_count(),
            before + 1,
            "remainder > 2 KB should produce a new free node"
        );
    }

    /// A remainder at or below 2 KB must not create a new node — those bytes
    /// are left as unused padding in an existing node
    #[test]
    fn small_remainder_does_not_create_new_node() {
        let mut alloc = FreeListAllocator::new(2048);
        let before = alloc.node_count();
        // Leave exactly 2048 bytes remaining — not strictly greater than the threshold.
        alloc.alloc_first(CHUNK_SIZE - 2048).unwrap();
        assert_eq!(
            alloc.node_count(),
            before,
            "remainder <= 2 KB should not create a new node"
        );
    }

    #[test]
    fn alloc_exceeding_chunk_size_fails() {
        let mut alloc = FreeListAllocator::new(2048);
        let result = alloc.alloc_first(CHUNK_SIZE + 1);
        assert!(matches!(result, Err(FreeListAllocError::NoRoomLeft(_, _))));
    }

    /// Once all free space is consumed every further allocation must fail.
    #[test]
    fn alloc_after_filling_fails() {
        let mut alloc = FreeListAllocator::new(2048);
        let half = CHUNK_SIZE / 2;
        alloc.alloc_first(half).unwrap();
        alloc.alloc_first(half).unwrap();
        let result = alloc.alloc_first(half);
        assert!(matches!(result, Err(FreeListAllocError::NoRoomLeft(_, _))));
    }
}
