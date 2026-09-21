use crate::renderer::{
    GPUAllocationHandle,
    gpu_allocator::allocation_tables::{AllocationSlot, AllocationTableError, TAllocationTable},
};
use std::collections::HashMap;

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
pub(in crate::renderer) struct AssetAlocationTable {
    free_list: Vec<usize>,
    meta: Vec<AssetAllocationMeta>,
    table: HashMap<GPUAllocationHandle, usize>,
}

impl TAllocationTable for AssetAlocationTable {
    type Handle = GPUAllocationHandle;
    type MetaData = AssetAllocationMeta;

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

    fn dealloc(&mut self, handle: &Self::Handle) -> Result<Self::MetaData, AllocationTableError> {
        todo!()
    }

    #[cfg(test)]
    fn get_table(&self) -> HashMap<Self::Handle, usize> {
        todo!()
    }

    fn new() -> Self {
        Self {
            free_list: Vec::new(),
            meta: Vec::new(),
            table: HashMap::new(),
        }
    }
}
