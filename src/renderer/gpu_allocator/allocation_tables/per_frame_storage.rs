use crate::renderer::{
    AllocationTableError, GPUInstanceHandle,
    gpu_allocator::{
        GPUUploadResult,
        allocation_tables::{
            AllocationSlot, TAllocationTable, asset_alloc_table::AssetAllocationMeta,
        },
    },
};

impl AllocationSlot for () {
    fn new(chunk_id: usize, node_id: usize) -> Self {
        ()
    }

    fn chunk(&self) -> usize {
        panic!()
    }

    fn node(&self) -> usize {
        panic!()
    }
}

pub(in crate::renderer) struct FrameStorageDataTable {}
impl TAllocationTable for FrameStorageDataTable {
    type Handle = GPUInstanceHandle;

    type MetaData = ();

    fn new() -> Self {
        Self {}
    }

    fn allocate(&mut self, handle: Self::Handle, upload_meta: Self::MetaData) -> usize {
        0
    }

    fn resolve(&self, handle: &Self::Handle) -> Option<Self::MetaData> {
        Some(())
    }

    fn dealloc(&mut self, handle: &Self::Handle) -> Result<Self::MetaData, AllocationTableError> {
        todo!()
    }

    #[cfg(test)]
    fn get_table(&self) -> std::collections::HashMap<Self::Handle, usize> {
        todo!()
    }
}
