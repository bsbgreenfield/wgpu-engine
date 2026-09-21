use std::collections::HashMap;

use crate::renderer::{
    AllocationTableError, GPUInstanceHandle, PrototypeHandle,
    gpu_allocator::allocation_tables::{
        AllocationSlot, TAllocationTable, asset_alloc_table::AssetAllocationMeta,
    },
};

pub(in crate::renderer::gpu_allocator) trait InstanceAllocationTable:
    TAllocationTable<Handle = GPUInstanceHandle>
{
    fn get_prototype_meta(&self, handle: &GPUInstanceHandle) -> (usize, usize);
    fn release(
        &mut self,
        handle: &GPUInstanceHandle,
    ) -> Result<Option<Self::MetaData>, AllocationTableError>;
}

#[derive(Default)]
pub(in crate::renderer) struct SharedInstanceAllocTable {
    meta: Vec<SharedInstanceAllocationSlot>,
    table: HashMap<GPUInstanceHandle, usize>,
    prototype_registry: HashMap<PrototypeHandle, usize>,
}
impl InstanceAllocationTable for SharedInstanceAllocTable {
    fn release(
        &mut self,
        handle: &GPUInstanceHandle,
    ) -> Result<Option<Self::MetaData>, AllocationTableError> {
        todo!()
    }

    fn get_prototype_meta(&self, handle: &GPUInstanceHandle) -> (usize, usize) {
        let prototype_slot = self
            .prototype_registry
            .get(&handle.prototype)
            .expect("could not find registered prototype");
        let meta = &self.meta[*prototype_slot];
        (meta.chunk(), meta.node())
    }
}

#[derive(Clone)]
pub enum SharedInstanceAllocationSlot {
    Prototype { slot: AssetAllocationMeta },
    Shared,
    Copied { slot: AssetAllocationMeta },
}

impl AllocationSlot for SharedInstanceAllocationSlot {
    fn new(chunk_id: usize, node_id: usize) -> Self {
        todo!()
    }

    fn chunk(&self) -> usize {
        match self {
            Self::Prototype { slot, .. } => slot.chunk(),
            Self::Copied { slot, .. } => slot.chunk(),
            _ => unreachable!(),
        }
    }

    fn node(&self) -> usize {
        match self {
            Self::Prototype { slot, .. } => slot.node(),
            Self::Copied { slot, .. } => slot.node(),
            _ => unreachable!(),
        }
    }
}

impl TAllocationTable for SharedInstanceAllocTable {
    type Handle = GPUInstanceHandle;

    type MetaData = SharedInstanceAllocationSlot;

    fn new() -> Self {
        Self::default()
    }

    fn allocate(&mut self, handle: Self::Handle, upload_meta: Self::MetaData) {
        match &upload_meta {
            SharedInstanceAllocationSlot::Prototype { .. } => {
                if let Some(meta_idx) = self.prototype_registry.get(&handle.prototype) {
                    self.table.insert(handle, *meta_idx);
                } else {
                    self.prototype_registry
                        .insert(handle.prototype, self.meta.len());
                    self.meta.push(upload_meta.clone());
                }
            }
            SharedInstanceAllocationSlot::Shared => {
                let prototype_slot = *self
                    .prototype_registry
                    .get(&handle.prototype)
                    .expect("cannot share a prototype tha doesnt exist");
                self.table.insert(handle, prototype_slot);
            }
            SharedInstanceAllocationSlot::Copied { .. } => {
                assert!(
                    self.prototype_registry.contains_key(&handle.prototype),
                    "Copy slot for a prototype that is not registered here"
                );
                self.table.insert(handle, self.meta.len());
                self.meta.push(upload_meta.clone());
            }
        }
    }

    fn resolve(&self, handle: &Self::Handle) -> Option<Self::MetaData> {
        self.table
            .get(handle)
            .map(|idx| self.meta.get(*idx).cloned().unwrap())
    }

    fn dealloc(&mut self, handle: &Self::Handle) -> Result<Self::MetaData, AllocationTableError> {
        todo!()
    }

    #[cfg(test)]
    fn get_table(&self) -> std::collections::HashMap<Self::Handle, usize> {
        todo!()
    }
}
