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

struct PrototypeSlot {
    meta_idx: usize,
    ref_count: usize,
}
#[derive(Default)]
pub(in crate::renderer) struct SharedInstanceAllocTable {
    meta: Vec<SharedInstanceAllocationSlot>,
    table: HashMap<GPUInstanceHandle, usize>,
    prototype_registry: HashMap<PrototypeHandle, PrototypeSlot>,
    free_list: Vec<usize>,
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
        let meta = &self.meta[prototype_slot.meta_idx];
        (meta.chunk(), meta.node())
    }
}

#[derive(Clone)]
pub enum SharedInstanceAllocationSlot {
    Shared { slot: AssetAllocationMeta },
    Copied { slot: AssetAllocationMeta },
}

impl AllocationSlot for SharedInstanceAllocationSlot {
    fn new(chunk_id: usize, node_id: usize) -> Self {
        todo!()
    }

    fn chunk(&self) -> usize {
        match self {
            Self::Shared { slot, .. } => slot.chunk(),
            Self::Copied { slot, .. } => slot.chunk(),
        }
    }

    fn node(&self) -> usize {
        match self {
            Self::Shared { slot, .. } => slot.node(),
            Self::Copied { slot, .. } => slot.node(),
        }
    }
}

impl TAllocationTable for SharedInstanceAllocTable {
    type Handle = GPUInstanceHandle;

    type MetaData = SharedInstanceAllocationSlot;

    fn new() -> Self {
        Self::default()
    }

    fn allocate(&mut self, handle: Self::Handle, upload_meta: Self::MetaData) -> usize {
        match &upload_meta {
            SharedInstanceAllocationSlot::Shared { slot } => {
                // if the prototype has already been uploaded
                let meta_idx = if let Some(PrototypeSlot {
                    meta_idx,
                    ref_count,
                }) = self.prototype_registry.get_mut(&handle.prototype)
                {
                    let SharedInstanceAllocationSlot::Shared { .. } = &mut self.meta[*meta_idx]
                    else {
                        panic!();
                    };
                    *ref_count += 1;
                    *meta_idx
                } else {
                    match self.free_list.pop() {
                        Some(free_idx) => {
                            self.meta.insert(free_idx, upload_meta);
                            free_idx
                        }
                        None => {
                            self.meta
                                .push(SharedInstanceAllocationSlot::Shared { slot: slot.clone() });
                            self.prototype_registry.insert(
                                handle.prototype.clone(),
                                PrototypeSlot {
                                    meta_idx: self.meta.len() - 1,
                                    ref_count: 1,
                                },
                            );
                            self.meta.len() - 1
                        }
                    }
                };
                self.table.insert(handle, meta_idx);
                meta_idx
            }
            SharedInstanceAllocationSlot::Copied { slot } => {
                let prototype_slot = self
                    .prototype_registry
                    .get_mut(&handle.prototype)
                    .expect("cant copy if there isnt a prototype");
                prototype_slot.ref_count += 1;
                match self.free_list.pop() {
                    Some(free_idx) => {
                        self.meta.insert(free_idx, upload_meta);
                        free_idx
                    }
                    None => {
                        self.meta
                            .push(SharedInstanceAllocationSlot::Copied { slot: slot.clone() });
                        self.table.insert(handle, self.meta.len() - 1);
                        self.meta.len() - 1
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

    fn dealloc(&mut self, handle: &Self::Handle) -> Result<Self::MetaData, AllocationTableError> {
        todo!()
    }

    #[cfg(test)]
    fn get_table(&self) -> std::collections::HashMap<Self::Handle, usize> {
        todo!()
    }
}
