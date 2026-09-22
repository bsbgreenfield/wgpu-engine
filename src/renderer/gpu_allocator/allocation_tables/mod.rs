#[cfg(test)]
use std::collections::HashMap;

use crate::{
    renderer::{
        AllocationTableError, GPUInstanceHandle, PrototypeHandle,
        gpu_allocator::{
            GPUUploadable, allocation_tables::shared_instance_alloc_table::SharedInstanceAllocTable,
        },
    },
    util::types::{
        GlobalTransform, InstanceOffset, InstanceRecordData, InverseBindMatrix, JointTransform,
        LocalTransform,
    },
};

pub(super) mod asset_alloc_table;
pub(super) mod shared_instance_alloc_table;
pub(super) mod texture_alloc_table;

pub(in crate::renderer) trait StorageData:
    bytemuck::Pod + std::fmt::Debug + Sized
{
    const BUFFER_USAGES: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE
        .union(wgpu::BufferUsages::COPY_DST)
        .union(wgpu::BufferUsages::COPY_SRC);
}

pub trait SharedInstanceData:
    StorageData + GPUUploadable<GPUHandle = GPUInstanceHandle, AllocTable = SharedInstanceAllocTable>
{
}

impl SharedInstanceData for LocalTransform {}
impl SharedInstanceData for JointTransform {}
impl SharedInstanceData for InverseBindMatrix {}

impl StorageData for LocalTransform {}
impl StorageData for JointTransform {}
impl StorageData for InverseBindMatrix {}

impl StorageData for GlobalTransform {}
impl StorageData for InstanceRecordData {}
impl StorageData for InstanceOffset {}

pub trait AllocationSlot: Clone {
    fn new(chunk_id: usize, node_id: usize) -> Self;
    fn chunk(&self) -> usize;
    fn node(&self) -> usize;
}
pub trait TAllocationTable {
    type Handle: Eq + std::hash::Hash + Clone + std::fmt::Debug;
    type MetaData: AllocationSlot;

    fn new() -> Self;
    fn allocate(&mut self, handle: Self::Handle, upload_meta: Self::MetaData);

    fn resolve(&self, handle: &Self::Handle) -> Option<Self::MetaData>;

    fn dealloc(
        &mut self,
        handle: &Self::Handle,
    ) -> Result<Option<Self::MetaData>, AllocationTableError>;

    #[cfg(test)]
    fn get_table(&self) -> HashMap<Self::Handle, usize>;
}
