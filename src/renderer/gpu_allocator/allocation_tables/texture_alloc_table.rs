use std::collections::HashMap;

use crate::renderer::{
    GPUAllocationHandle,
    gpu_allocator::allocation_tables::{TAllocationTable, asset_alloc_table::AssetAllocationMeta},
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
    alloc_meta: Vec<AssetAllocationMeta>,
    table: HashMap<GPUAllocationHandle, Vec<usize>>,
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
                slots.push(handle.index);
            }
            None => {
                self.alloc_meta.push(upload_meta);
                self.table
                    .insert(handle.asset_handle, vec![self.alloc_meta.len() - 1]);
                ()
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
    ) -> Result<Option<Self::MetaData>, crate::renderer::AllocationTableError> {
        todo!()
    }

    #[cfg(test)]
    fn get_table(&self) -> HashMap<Self::Handle, usize> {
        todo!()
    }
}
