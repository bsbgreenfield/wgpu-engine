use crate::renderer::{
    GPUAllocationHandle,
    gpu_allocator::allocation_tables::{AllocationSlot, AllocationTableError, TAllocationTable},
};
use std::{collections::HashMap, fmt::Debug, hash::Hash};

#[derive(Clone)]
pub struct AssetAllocationMeta {
    chunk_id: u32,
    node_id: u32,
}
impl AssetAllocationMeta {
    pub(super) fn new(chunk: usize, node: usize) -> Self {
        Self {
            chunk_id: chunk as u32,
            node_id: node as u32,
        }
    }
}

impl AllocationSlot for AssetAllocationMeta {
    fn chunk(&self) -> usize {
        self.chunk_id as usize
    }

    fn node(&self) -> usize {
        self.node_id as usize
    }

    fn new(chunk_id: usize, node_id: usize) -> Self {
        Self {
            chunk_id: chunk_id as u32,
            node_id: node_id as u32,
        }
    }
}
pub(in crate::renderer) struct SingleAlocationTable<H: Eq + Hash + Clone + Debug> {
    table: HashMap<H, AssetAllocationMeta>,
}

impl<H: Eq + Hash + Clone + Debug> TAllocationTable for SingleAlocationTable<H> {
    type Handle = H;
    type MetaData = AssetAllocationMeta;

    fn allocate(&mut self, handle: Self::Handle, upload_meta: Self::MetaData) {
        self.table.insert(handle, upload_meta);
    }

    fn resolve(&self, handle: &Self::Handle) -> Option<Self::MetaData> {
        self.table.get(handle).cloned()
    }

    fn dealloc(&mut self, handle: &Self::Handle) -> Result<Self::MetaData, AllocationTableError> {
        todo!()
    }

    #[cfg(test)]
    fn get_table(&self) -> HashMap<Self::Handle, usize> {
        todo!()
    }

    fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }
}
