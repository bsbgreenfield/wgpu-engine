use std::collections::HashMap;

use crate::app::renderer::{gpu_allocator::AllocMetaData, renderer::GPUInstanceHandle};

pub(super) struct AllocationTable {
    free_list: Vec<usize>,
    alloc_meta: Vec<AllocMetaData>,
    table: HashMap<GPUInstanceHandle, usize>,
}

impl AllocationTable {
    pub(super) fn get_meta(&mut self, slot_idx: usize) -> Option<&mut AllocMetaData> {
        self.alloc_meta.get_mut(slot_idx)
    }

    pub(super) fn register_instance(
        &mut self,
        gpu_instance_handle: GPUInstanceHandle,
        slot: usize,
    ) {
        self.alloc_meta[slot].ref_count += 1;
        self.table.insert(gpu_instance_handle, slot);
    }

    pub(super) fn allocate(
        &mut self,
        handle: GPUInstanceHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> usize {
        self.table.insert(handle, self.alloc_meta.len());
        let meta = AllocMetaData {
            chunk_id,
            node_id,
            ref_count: 1,
        };
        if let Some(free_idx) = self.free_list.pop() {
            self.alloc_meta[free_idx] = meta;
            free_idx
        } else {
            self.alloc_meta.push(meta);
            self.alloc_meta.len() - 1
        }
    }

    pub(super) fn dealloc(&mut self, handle: &GPUInstanceHandle) -> Option<()> {
        let idx = *self.table.get(handle)?;
        self.table.remove(handle);
        let meta = self.alloc_meta.get_mut(idx)?;
        meta.ref_count -= 1;
        if meta.ref_count == 0 {
            self.alloc_meta.remove(idx);
        }
        self.free_list.push(idx);
        Some(())
    }

    pub(super) fn resolve(&self, handle: &GPUInstanceHandle) -> Option<&AllocMetaData> {
        let idx = self.table.get(handle)?;
        self.alloc_meta.get(*idx)
    }

    pub(super) fn new() -> Self {
        Self {
            free_list: vec![],
            alloc_meta: vec![],
            table: HashMap::new(),
        }
    }
}
