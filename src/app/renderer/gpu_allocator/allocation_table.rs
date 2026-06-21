use std::collections::HashMap;

use crate::app::renderer::{gpu_allocator::AllocMetaData, renderer::GPUInstanceHandle};

pub(super) struct AllocationTable {
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
        self.table.insert(gpu_instance_handle, slot);
    }

    pub(super) fn allocate(
        &mut self,
        handle: GPUInstanceHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> usize {
        self.table.insert(handle, self.alloc_meta.len());
        self.alloc_meta.push(AllocMetaData {
            chunk_id,
            node_id,
            ref_count: 1,
        });
        self.alloc_meta.len() - 1
    }

    pub(super) fn resolve(&self, handle: &GPUInstanceHandle) -> Option<&AllocMetaData> {
        let idx = self.table.get(handle)?;
        self.alloc_meta.get(*idx)
    }

    pub(super) fn new() -> Self {
        Self {
            alloc_meta: vec![],
            table: HashMap::new(),
        }
    }
}
