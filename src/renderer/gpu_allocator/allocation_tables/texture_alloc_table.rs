use std::collections::HashMap;

use crate::renderer::{
    GPUAllocationHandle,
    gpu_allocator::allocation_tables::{
        AllocationTableError, TAllocationTable, asset_alloc_table::AssetAllocationMeta,
    },
};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct GPUTextureHandle {
    asset_handle: GPUAllocationHandle,
    index: usize,
}

impl GPUTextureHandle {
    pub fn new(alloc: GPUAllocationHandle, index: usize) -> Self {
        Self {
            asset_handle: alloc,
            index,
        }
    }
}

#[derive(Default)]
pub struct TextureAllocTable {
    free_list: Vec<usize>,
    alloc_meta: Vec<AssetAllocationMeta>,
    table: HashMap<GPUAllocationHandle, Vec<usize>>,
}
impl TextureAllocTable {
    pub fn dealloc_all(
        &mut self,
        alloc_handle: &GPUAllocationHandle,
    ) -> Result<Vec<AssetAllocationMeta>, AllocationTableError> {
        let mut res = Vec::new();
        let indices = self
            .table
            .remove(alloc_handle)
            .ok_or(AllocationTableError::AllocationNotFound)?;
        for idx in indices.iter().filter(|i| **i != usize::MAX) {
            self.free_list.push(*idx);
            res.push(self.alloc_meta[*idx].clone());
        }
        Ok(res)
    }
}
impl TAllocationTable for TextureAllocTable {
    type Handle = GPUTextureHandle;

    type MetaData = AssetAllocationMeta;

    fn new() -> Self {
        Self::default()
    }

    fn allocate(&mut self, handle: Self::Handle, upload_meta: Self::MetaData) {
        match self.table.get_mut(&handle.asset_handle) {
            Some(slots) => {
                if slots.len() - 1 < handle.index {
                    slots.resize(handle.index + 1, usize::MAX);
                }
                match self.free_list.pop() {
                    Some(free_idx) => {
                        slots[handle.index] = free_idx;
                        self.alloc_meta[free_idx] = upload_meta;
                    }
                    None => {
                        slots[handle.index] = self.alloc_meta.len();
                        self.alloc_meta.push(upload_meta);
                    }
                };
            }
            None => {
                let mut slots = Vec::from_iter(std::iter::repeat_n(usize::MAX, handle.index + 1));
                match self.free_list.pop() {
                    Some(free_idx) => {
                        slots[handle.index] = free_idx;
                        self.table.insert(handle.asset_handle, slots);
                        self.alloc_meta[free_idx] = upload_meta;
                    }
                    None => {
                        slots[handle.index] = self.alloc_meta.len();
                        self.table.insert(handle.asset_handle, slots);
                        self.alloc_meta.push(upload_meta);
                    }
                };
            }
        }
    }

    fn resolve(&self, handle: &Self::Handle) -> Option<Self::MetaData> {
        let slot = *self.table.get(&handle.asset_handle)?.get(handle.index)?;
        self.alloc_meta.get(slot).cloned()
    }

    fn dealloc(
        &mut self,
        handle: &Self::Handle,
    ) -> Result<Option<Self::MetaData>, AllocationTableError> {
        let alloc = &handle.asset_handle;
        let indices = self
            .table
            .remove(alloc)
            .ok_or(AllocationTableError::AllocationNotFound)?;
        for idx in indices {
            self.free_list.push(idx);
        }

        Ok(None)
    }

    #[cfg(test)]
    fn get_table(&self) -> HashMap<Self::Handle, usize> {
        todo!()
    }
}
