use std::range::Range;

use crate::renderer::gpu_allocator::{CHUNK_SIZE, FreeListAllocError};

pub(super) struct FreeListAllocator {
    free_nodes: Vec<usize>,
    nodes: Vec<FreeListNode>,
    chunk_size: u32,
    head: usize,
    minimum_node_size: usize,
}

struct FreeListNode {
    block_size: u32,
    occupied: bool,
    offset: u32,
    next: Option<u32>,
    prev: Option<u32>,
}

impl<'chunk> FreeListNode {
    fn new(block_size: u32, next: Option<u32>, prev: Option<u32>, offset: u32) -> Self {
        Self {
            block_size,
            occupied: false,
            next,
            prev,
            offset,
        }
    }
}

impl FreeListAllocator {
    pub(super) fn dealloc(&mut self, node_id: usize) -> Result<(), FreeListAllocError> {
        let merge_prev = if let Some(prev) = self.nodes[node_id].prev {
            !self.nodes[prev as usize].occupied
        } else {
            false
        };
        let merge_next = if let Some(next) = self.nodes[node_id].next {
            !self.nodes[next as usize].occupied
        } else {
            false
        };

        self.nodes
            .get_mut(node_id)
            .ok_or(FreeListAllocError::NodeNotFount(node_id))?
            .occupied = false;

        let curr_node_id: u32 = if merge_prev {
            let (prev_id, next_id): (u32, Option<u32>) = {
                let n = self.nodes.get_mut(node_id).unwrap();
                (n.prev.unwrap(), n.next)
            };
            let block_size = self.remove_node(node_id);
            let n = self.nodes.get_mut(prev_id as usize).unwrap();
            n.next = next_id;
            n.block_size += block_size;
            if next_id.is_some() {
                self.nodes.get_mut(next_id.unwrap() as usize).unwrap().prev = Some(prev_id);
            }
            prev_id
        } else {
            node_id as u32
        };
        if merge_next {
            let next_id = {
                let node = self.nodes.get_mut(curr_node_id as usize).unwrap();
                node.next.unwrap()
            };
            let next_next_id: Option<u32> = { self.nodes.get_mut(next_id as usize).unwrap().next };
            let block_size = self.remove_node(next_id as usize);

            let curr_node = self.nodes.get_mut(curr_node_id as usize).unwrap();
            curr_node.block_size += block_size;

            curr_node.next = next_next_id;
        }

        Ok(())
    }

    #[inline]
    pub(super) fn resolve(&self, node_id: usize) -> Range<u32> {
        let node = &self.nodes[node_id];
        Range::from(node.offset..node.offset + node.block_size)
    }

    #[inline]
    pub(super) fn offset_of(&self, node_id: usize) -> u64 {
        self.nodes[node_id].offset as u64
    }
    pub(super) fn new(chunk_size: u32, minimum_node_size: usize) -> Self {
        Self {
            free_nodes: vec![],
            nodes: vec![FreeListNode::new(chunk_size, None, None, 0)],
            chunk_size: chunk_size,
            head: 0,
            minimum_node_size,
        }
    }

    fn find_first(&self, size: u32) -> Result<(usize, u32), FreeListAllocError> {
        if size > self.chunk_size {
            return Err(FreeListAllocError::NoRoomLeft(size, CHUNK_SIZE));
        }
        let mut offset = 0;
        let mut node_idx = self.head as u32;
        let mut debug_max_avail_node_size = 0;

        loop {
            let node = &self.nodes[node_idx as usize];

            // if the node is available and large enough, use this node
            if !node.occupied && node.block_size >= size {
                if node.block_size > debug_max_avail_node_size {
                    debug_max_avail_node_size = node.block_size;
                }
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
            debug_max_avail_node_size,
        ))
    }

    #[cfg(test)]
    fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn alloc_first(&mut self, size: u32) -> Result<usize, FreeListAllocError> {
        // TODO: account for alignemnt and padding
        let (offset, node_idx) = self.find_first(size)?;
        let node = &mut self.nodes[node_idx as usize];
        let remaining_node_space = node.block_size - size; // space remaining in this node after allocation
        node.occupied = true;

        // if this allocation has created a new empty node directly after it with at least 2 kb, mark this empty space
        // as a new node, add it to the list, and set it as .next for the selected node
        if remaining_node_space > self.minimum_node_size as u32 {
            node.block_size -= remaining_node_space;
            let new_node = FreeListNode::new(
                remaining_node_space,
                self.nodes[node_idx as usize].next,
                Some(node_idx),
                offset as u32 + size,
            );
            let slot = self.add_node(new_node);
            self.nodes[node_idx as usize].next = Some(slot as u32);
        }
        // if there is not enough space for a new node, do nothing

        Ok(node_idx as usize)
    }

    fn add_node(&mut self, node: FreeListNode) -> usize {
        if let Some(free) = self.free_nodes.pop() {
            self.nodes[free] = node;
            free
        } else {
            self.nodes.push(node);
            self.nodes.len() - 1
        }
    }
    fn remove_node(&mut self, node_id: usize) -> u32 {
        self.free_nodes.push(node_id);
        self.nodes[node_id].block_size
    }
}

#[cfg(test)]
mod free_list_tests {
    use super::*;

    const ALLOC: u32 = 1024 * 64; // 64 KB

    #[test]
    fn first_alloc_offset_is_zero() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let id = alloc.alloc_first(ALLOC).unwrap();
        assert_eq!(alloc.offset_of(id), 0);
    }

    #[test]
    fn resolve_returns_correct_range() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let id = alloc.alloc_first(ALLOC).unwrap();
        let range = alloc.resolve(id);
        assert_eq!(range.start, 0);
        assert_eq!(range.end, ALLOC);
    }

    // ── sequential allocation offsets ─────────────────────────────────────────

    #[test]
    fn second_alloc_starts_after_first() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
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
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
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
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
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
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
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
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
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
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let result = alloc.alloc_first(CHUNK_SIZE + 1);
        assert!(matches!(result, Err(FreeListAllocError::NoRoomLeft(_, _))));
    }

    /// Once all free space is consumed every further allocation must fail.
    #[test]
    fn alloc_after_filling_fails() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let half = CHUNK_SIZE / 2;
        alloc.alloc_first(half).unwrap();
        alloc.alloc_first(half).unwrap();
        let result = alloc.alloc_first(half);
        assert!(matches!(result, Err(FreeListAllocError::NoRoomLeft(_, _))));
    }

    // ── prev pointer wiring ────────────────────────────────────────────────

    /// The remainder created by an allocation must link back to its predecessor.
    #[test]
    fn alloc_sets_prev_on_remainder_node() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let id = alloc.alloc_first(ALLOC).unwrap();
        let remainder_idx = alloc.nodes[id]
            .next
            .expect("a remainder node should exist after the first allocation")
            as usize;
        assert_eq!(
            alloc.nodes[remainder_idx].prev,
            Some(id as u32),
            "remainder node's prev should point back to the allocated node"
        );
    }

    /// Consecutive allocations should form a doubly-linked chain in both
    /// directions.
    #[test]
    fn sequential_allocs_form_prev_chain() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let a = alloc.alloc_first(ALLOC).unwrap();
        let b = alloc.alloc_first(ALLOC).unwrap();
        let c = alloc.alloc_first(ALLOC).unwrap();
        assert_eq!(alloc.nodes[a].prev, None, "head allocation has no prev");
        assert_eq!(alloc.nodes[b].prev, Some(a as u32));
        assert_eq!(alloc.nodes[c].prev, Some(b as u32));
    }

    // ── dealloc basics ─────────────────────────────────────────────────────

    /// Freeing a single node between two occupied neighbors must mark it
    /// unoccupied without touching the neighbors.
    #[test]
    fn dealloc_marks_node_unoccupied() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let a = alloc.alloc_first(ALLOC).unwrap();
        let b = alloc.alloc_first(ALLOC).unwrap();
        let c = alloc.alloc_first(ALLOC).unwrap();
        alloc.dealloc(b).unwrap();
        assert!(!alloc.nodes[b].occupied, "deallocated node should be free");
        assert!(alloc.nodes[a].occupied, "left neighbor must stay occupied");
        assert!(alloc.nodes[c].occupied, "right neighbor must stay occupied");
    }

    /// With both neighbors occupied there is nothing to coalesce, so the freed
    /// node should keep its original size and no slot should be recycled.
    #[test]
    fn dealloc_with_occupied_neighbors_does_not_merge() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let _a = alloc.alloc_first(ALLOC).unwrap();
        let b = alloc.alloc_first(ALLOC).unwrap();
        let _c = alloc.alloc_first(ALLOC).unwrap();
        let free_before = alloc.free_nodes.len();
        alloc.dealloc(b).unwrap();
        assert_eq!(
            alloc.free_nodes.len(),
            free_before,
            "no slot should be recycled when both neighbors are occupied"
        );
        assert_eq!(
            alloc.nodes[b].block_size, ALLOC,
            "block size must be unchanged when no merging happens"
        );
    }

    /// Reallocating after a dealloc should reuse the freed offset.
    #[test]
    fn realloc_after_dealloc_reuses_offset() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let a = alloc.alloc_first(ALLOC).unwrap();
        let _b = alloc.alloc_first(ALLOC).unwrap();
        let offset_a = alloc.offset_of(a);
        alloc.dealloc(a).unwrap();
        let new_id = alloc.alloc_first(ALLOC).unwrap();
        assert_eq!(
            alloc.offset_of(new_id),
            offset_a,
            "reallocated block should reuse the deallocated offset"
        );
    }

    // ── merging during dealloc ─────────────────────────────────────────────

    /// When the previous node is already free, dealloc should fold the
    /// just-freed node's space into it.
    #[test]
    fn dealloc_merges_with_free_prev() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let a = alloc.alloc_first(ALLOC).unwrap();
        let b = alloc.alloc_first(ALLOC).unwrap();
        let c = alloc.alloc_first(ALLOC).unwrap();
        alloc.dealloc(a).unwrap();
        let free_before = alloc.free_nodes.len();
        alloc.dealloc(b).unwrap();
        assert_eq!(
            alloc.free_nodes.len(),
            free_before + 1,
            "the merged-away node's slot should be recycled"
        );
        assert_eq!(
            alloc.nodes[a].block_size,
            ALLOC * 2,
            "free prev node should absorb the deallocated node's space"
        );
        // prev and next are wired up properly
        assert_eq!(alloc.nodes[a].next, Some(c as u32));
        assert_eq!(alloc.nodes[c].prev, Some(a as u32));
        assert!(!alloc.nodes[a].occupied);

        let new = alloc.alloc_first(ALLOC * 2).unwrap();
        assert_eq!(alloc.offset_of(new), 0);
        assert_eq!(alloc.nodes[new].block_size, ALLOC * 2);
        // c is untouched and still reachable from the merged node's next pointer.
        assert!(alloc.nodes[c].occupied);
        assert_eq!(alloc.offset_of(c), (ALLOC * 2) as u64);
        assert_eq!(alloc.nodes[new].next, Some(c as u32));
    }

    /// When the next node is already free, dealloc should absorb its space
    /// into the just-freed node.
    #[test]
    fn dealloc_merges_with_free_next() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let a = alloc.alloc_first(ALLOC).unwrap();
        let b = alloc.alloc_first(ALLOC).unwrap();
        let c = alloc.alloc_first(ALLOC).unwrap();
        // Freeing c first folds the tail remainder into c's slot, so the
        // node that sits to b's right has block_size = CHUNK_SIZE - 2*ALLOC.
        alloc.dealloc(c).unwrap();
        let free_before = alloc.free_nodes.len();
        alloc.dealloc(b).unwrap();
        assert_eq!(
            alloc.free_nodes.len(),
            free_before + 1,
            "the merged-away next node's slot should be recycled"
        );

        // prev and next are wired up properly
        assert_eq!(alloc.nodes[a].next, Some(b as u32));
        assert_eq!(alloc.nodes[b].prev, Some(a as u32));

        assert!(!alloc.nodes[b].occupied);
        assert_eq!(
            alloc.nodes[b].block_size,
            CHUNK_SIZE - ALLOC,
            "deallocated node should absorb the free next node (which already \
             swallowed the tail remainder)"
        );
        let new = alloc.alloc_first(CHUNK_SIZE - ALLOC).unwrap();
        assert_eq!(alloc.offset_of(new), ALLOC as u64);
        assert_eq!(alloc.nodes[new].block_size, CHUNK_SIZE - ALLOC);
    }

    /// Freeing every allocation in turn must leave the chunk represented by a
    /// single free node spanning the full CHUNK_SIZE — exercising the
    /// merge-with-both-neighbors path.
    #[test]
    fn dealloc_merges_with_both_neighbors() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let a = alloc.alloc_first(ALLOC).unwrap();
        let b = alloc.alloc_first(ALLOC).unwrap();
        let c = alloc.alloc_first(ALLOC).unwrap();
        alloc.dealloc(a).unwrap();
        alloc.dealloc(c).unwrap();
        alloc.dealloc(b).unwrap();
        assert!(!alloc.nodes[a].occupied);
        assert_eq!(
            alloc.nodes[a].block_size, CHUNK_SIZE,
            "after freeing every allocation, the head node should describe the \
             entire chunk as one contiguous free region"
        );
        let new = alloc.alloc_first(CHUNK_SIZE).unwrap();
        assert_eq!(alloc.offset_of(new), 0);
        assert_eq!(alloc.resolve(new), Range::from(0..CHUNK_SIZE));
    }

    /// Re-allocating two ALLOC-sized blocks back into a merged region should
    /// reuse the recycled slots and land contiguously before the surviving
    /// node — verifying the next-chain remains walkable after a merge.
    #[test]
    fn allocs_after_merge_are_contiguous_with_remaining_nodes() {
        let mut alloc = FreeListAllocator::new(CHUNK_SIZE, 2048);
        let a = alloc.alloc_first(ALLOC).unwrap();
        let b = alloc.alloc_first(ALLOC).unwrap();
        let c = alloc.alloc_first(ALLOC).unwrap();
        alloc.dealloc(a).unwrap();
        alloc.dealloc(b).unwrap();
        let new_a = alloc.alloc_first(ALLOC).unwrap();
        let new_b = alloc.alloc_first(ALLOC).unwrap();
        assert_eq!(alloc.offset_of(new_a), 0);
        assert_eq!(alloc.offset_of(new_b), ALLOC as u64);
        assert_eq!(alloc.offset_of(c), (ALLOC * 2) as u64);
        assert_eq!(
            alloc.nodes[new_a].next,
            Some(new_b as u32),
            "first re-alloc should link forward to the second"
        );
        assert_eq!(
            alloc.nodes[new_b].next,
            Some(c as u32),
            "second re-alloc should link forward to the surviving node c"
        );
    }
}
