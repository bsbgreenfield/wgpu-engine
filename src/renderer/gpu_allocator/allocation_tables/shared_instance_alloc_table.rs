use std::collections::HashMap;

use crate::renderer::{
    GPUInstanceHandle, PrototypeHandle,
    gpu_allocator::allocation_tables::{
        AllocationSlot, AllocationTableError, TAllocationTable,
        asset_alloc_table::AssetAllocationMeta,
    },
};

pub(in crate::renderer::gpu_allocator) trait InstanceAllocationTable:
    TAllocationTable<Handle = GPUInstanceHandle>
{
    fn get_prototype_meta(&self, handle: &GPUInstanceHandle) -> (usize, usize);
    fn release_prototype(
        &mut self,
        handle: &PrototypeHandle,
    ) -> Result<Option<Self::MetaData>, AllocationTableError>;
}

#[derive(Default)]
pub(in crate::renderer) struct SharedInstanceAllocTable {
    meta: Vec<SharedInstanceAllocationSlot>,
    table: HashMap<GPUInstanceHandle, usize>,
    prototype_registry: HashMap<PrototypeHandle, usize>,
    free_list: Vec<usize>,
}
impl InstanceAllocationTable for SharedInstanceAllocTable {
    fn release_prototype(
        &mut self,
        handle: &PrototypeHandle,
    ) -> Result<Option<Self::MetaData>, AllocationTableError> {
        if let Some(meta_slot) = self.prototype_registry.remove(handle) {
            self.free_list.push(meta_slot);
            return Ok(Some(
                self.meta
                    .get(meta_slot)
                    .ok_or(AllocationTableError::AllocationNotFound)?
                    .clone(),
            ));
        }
        Ok(None)
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
    fn new(_chunk_id: usize, _node_id: usize) -> Self {
        todo!("remove this function")
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
                    match self.free_list.pop() {
                        Some(free_idx) => {
                            self.prototype_registry.insert(handle.prototype, free_idx);
                            self.meta[free_idx] = upload_meta.clone();
                        }
                        None => {
                            self.prototype_registry
                                .insert(handle.prototype, self.meta.len());
                            self.meta.push(upload_meta.clone());
                        }
                    }
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
                match self.free_list.pop() {
                    Some(free_idx) => {
                        self.table.insert(handle, free_idx);
                        self.meta[free_idx] = upload_meta.clone();
                    }
                    None => {
                        self.table.insert(handle, self.meta.len());
                        self.meta.push(upload_meta.clone());
                    }
                }
            }
        }
    }

    fn resolve(&self, handle: &Self::Handle) -> Option<Self::MetaData> {
        self.table
            .get(handle)
            .map(|idx| self.meta.get(*idx).cloned().unwrap())
    }

    fn dealloc(
        &mut self,
        handle: &Self::Handle,
    ) -> Result<Option<Self::MetaData>, AllocationTableError> {
        let meta_idx = self
            .table
            .get(handle)
            .ok_or(AllocationTableError::AllocationNotFound)?;
        match self.meta.get(*meta_idx).expect("meta index not available") {
            SharedInstanceAllocationSlot::Prototype { slot: _ } => {
                // this is a shared instance, remove the table entry
                self.table.remove(handle);
                Ok(None)
            }
            SharedInstanceAllocationSlot::Shared => unreachable!(),
            SharedInstanceAllocationSlot::Copied { slot } => {
                self.free_list.push(*meta_idx);
                self.table.remove(handle);
                Ok(Some(SharedInstanceAllocationSlot::Copied {
                    slot: slot.clone(),
                }))
            }
        }
    }

    #[cfg(test)]
    fn get_table(&self) -> std::collections::HashMap<Self::Handle, usize> {
        todo!()
    }
}
