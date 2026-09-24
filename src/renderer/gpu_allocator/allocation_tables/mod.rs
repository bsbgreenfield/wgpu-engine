#[cfg(test)]
use std::collections::HashMap;
use std::{hash::Hash, marker::PhantomData};

use crate::{
    renderer::{
        GPUInstanceHandle,
        gpu_allocator::{
            GPUUploadable, VertexArenaError,
            allocation_tables::{
                asset_alloc_table::SingleAlocationTable,
                shared_instance_alloc_table::SharedInstanceAllocTable,
            },
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

pub(in crate::renderer) trait ReservedSlotData:
    StorageData
    + bytemuck::Pod
    + GPUUploadable<
        GPUHandle = GPUInstanceHandle,
        AllocTable = SingleAlocationTable<GPUInstanceHandle>,
    >
{
}

impl ReservedSlotData for InstanceRecordData {}

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

#[derive(Debug)]
pub enum AllocationTableError {
    AllocationNotFound,
    MaxAllocationReached(String),
    ProtoypeDeallocation,
    PrototypeReleaseFailed,
    DeallocationFailed,
    GPUArenaError(VertexArenaError),
}
impl std::fmt::Display for AllocationTableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AllocationTableError::AllocationNotFound => f.write_str("allocation not found"),
            AllocationTableError::MaxAllocationReached(al) => {
                write!(f, "Max allocation reach for: {}", al)
            }
            AllocationTableError::ProtoypeDeallocation => {
                f.write_str("tried to dealloc a prototype slot")
            }
            AllocationTableError::PrototypeReleaseFailed => {
                f.write_str("could not release prototype")
            }
            AllocationTableError::DeallocationFailed => f.write_str("dealloc error"),
            AllocationTableError::GPUArenaError(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for AllocationTableError {}
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

    #[allow(unused)]
    #[cfg(test)]
    fn get_table(&self) -> HashMap<Self::Handle, usize>;
}
