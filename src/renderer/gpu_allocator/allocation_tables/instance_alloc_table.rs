use std::collections::HashMap;

use crate::renderer::{
    GPUInstanceHandle,
    gpu_allocator::allocation_tables::{TAllocationTable, asset_alloc_table::AssetAllocationMeta},
};

pub(in crate::renderer) struct InstanceAllocTable {
    free_list: Vec<usize>,
    meta: Vec<AssetAllocationMeta>,
    table: HashMap<GPUInstanceHandle, usize>,
}

impl TAllocationTable for InstanceAllocTable {
    type Handle = GPUInstanceHandle;

    type MetaData = AssetAllocationMeta;

    fn new() -> Self {
        Self {
            free_list: Vec::new(),
            meta: Vec::new(),
            table: HashMap::<GPUInstanceHandle, usize>::new(),
        }
    }

    fn allocate(&mut self, handle: Self::Handle, upload_meta: Self::MetaData) -> usize {
        let slot = match self.free_list.pop() {
            Some(free_idx) => {
                self.meta[free_idx] = upload_meta;
                free_idx
            }
            None => {
                self.meta.push(upload_meta);
                self.meta.len() - 1
            }
        };
        self.table.insert(handle, slot);
        slot
    }

    fn resolve(&self, handle: &Self::Handle) -> Option<Self::MetaData> {
        let idx = self.table.get(handle)?;
        self.meta.get(*idx).map(|meta| meta.clone())
    }

    fn dealloc(
        &mut self,
        handle: &Self::Handle,
    ) -> Result<Self::MetaData, crate::renderer::AllocationTableError> {
        todo!()
    }

    #[cfg(test)]
    fn get_table(&self) -> HashMap<Self::Handle, usize> {
        todo!()
    }
}
