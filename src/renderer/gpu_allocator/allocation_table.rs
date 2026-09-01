use std::{collections::HashMap, hash::Hash};

use crate::renderer::gpu_allocator::{AllocMetaData, VertexArenaError};

pub(super) struct AllocationTable<H: Eq + Hash + Clone> {
    free_list: Vec<usize>,
    alloc_meta: Vec<AllocMetaData>,
    table: HashMap<H, usize>,
}

impl<H: Clone + Hash + Eq> AllocationTable<H> {
    pub(super) fn get_meta(&mut self, slot_idx: usize) -> Option<&mut AllocMetaData> {
        self.alloc_meta.get_mut(slot_idx)
    }

    pub(super) fn register_instance(&mut self, gpu_instance_handle: H, slot: usize) {
        self.alloc_meta[slot].ref_count += 1;
        self.table.insert(gpu_instance_handle, slot);
    }

    pub(super) fn allocate(&mut self, handle: H, chunk_id: usize, node_id: usize) -> usize {
        let meta = AllocMetaData {
            chunk_id,
            node_id,
            ref_count: 1,
        };
        let slot = match self.free_list.pop() {
            Some(free_idx) => {
                self.alloc_meta[free_idx] = meta;
                free_idx
            }
            None => {
                self.alloc_meta.push(meta);
                self.alloc_meta.len() - 1
            }
        };
        self.table.insert(handle, slot);
        slot
    }

    pub(super) fn remove(&mut self, handle: &H) -> Result<Option<AllocMetaData>, VertexArenaError> {
        let res: Option<AllocMetaData>;
        let Some(idx) = self.table.get(handle) else {
            return Ok(None);
        };
        let idx = *idx;
        self.table.remove(handle);
        let meta = self
            .alloc_meta
            .get_mut(idx)
            .ok_or(VertexArenaError::MetadataNotFound)?;
        meta.ref_count -= 1;
        if meta.ref_count == 0 {
            let goner = self.alloc_meta[idx].clone();
            self.free_list.push(idx);
            res = Some(goner);
        } else {
            return Ok(None);
        }
        Ok(res)
    }
    pub(super) fn dealloc(&mut self, handle: &H) -> Result<(), VertexArenaError> {
        let idx = *self
            .table
            .get(handle)
            .ok_or(VertexArenaError::AllocationSlotNotFound)?;
        self.table.remove(handle);
        let meta = self
            .alloc_meta
            .get_mut(idx)
            .ok_or(VertexArenaError::MetadataNotFound)?;
        meta.ref_count -= 1;
        Ok(())
    }

    pub(super) fn resolve(&self, handle: &H) -> Option<&AllocMetaData> {
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
