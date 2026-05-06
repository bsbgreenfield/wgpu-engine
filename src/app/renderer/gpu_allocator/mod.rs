use std::fmt::Debug;
use std::marker::PhantomData;
use std::{fmt::Display, ops::Range};

use bytemuck::Pod;
use std::error::Error;

use crate::app::renderer::InstanceUploadJob;
use crate::app::renderer::gpu_allocator::free_list::FreeListAllocator;
use crate::{
    app::renderer::GPUAllocationHandle, util::types::ModelVertex,
    world::instance_manager::InstanceHandle,
};

mod free_list;
pub(super) mod instance_arena;
pub(super) mod vertex_arena;

static MIMIMUM_INDEX_ALLOCATION_SIZE: usize = 1024;
static MIMIMUM_VERTEX_ALLOCATION_SIZE: usize = 2048;

static CHUNK_SIZE: u32 = 4_194_304;

#[derive(Debug)]
struct AllocMetaData {
    chunk_id: usize,
    node_id: usize,
}
impl AllocMetaData {
    fn new(chunk_id: usize, node_id: usize) -> Self {
        Self { chunk_id, node_id }
    }
}
struct GPUChunk<T: bytemuck::Pod + Debug> {
    remaining_space: u32,
    buffer: wgpu::Buffer,
    allocator: FreeListAllocator,
    _t: PhantomData<T>,
}
struct InstanceChunk<T: bytemuck::Pod + Debug> {
    remaining_space: u32,
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    allocator: FreeListAllocator,
    _t: PhantomData<T>,
}

impl<T: bytemuck::Pod + Debug> GPUChunk<T> {
    pub(super) fn gpu_alloc(
        &mut self,
        data: &[u8],
        queue: &wgpu::Queue,
        label: &str,
    ) -> Result<(usize, Range<u32>), VertexArenaError> {
        let size = (data.len() * size_of::<T>()) as u32;
        let node_idx: usize = if self.remaining_space >= size {
            self.allocator.alloc_first(size)?
        } else {
            return Err(VertexArenaError::DataTooLarge(size, label.to_string()));
        };
        // for datum in data.iter().take(10) {
        //     println!("{:?}", datum);
        // }
        let offset = self.allocator.offset_of(node_idx) as u32;
        queue.write_buffer(&self.buffer, offset.into(), data);
        Ok((node_idx, offset..offset + (data.len() as u32)))
    }
}
impl<T: bytemuck::Pod + Debug> InstanceChunk<T> {
    pub(super) fn gpu_alloc(
        &mut self,
        data: &[u8],
        queue: &wgpu::Queue,
        label: &str,
    ) -> Result<(usize, Range<u32>), VertexArenaError> {
        let size = (data.len() * size_of::<T>()) as u32;
        let node_idx: usize = if self.remaining_space >= size {
            self.allocator.alloc_first(size)?
        } else {
            return Err(VertexArenaError::DataTooLarge(size, label.to_string()));
        };
        // for datum in data.iter().take(10) {
        //     println!("{:?}", datum);
        // }
        let offset = self.allocator.offset_of(node_idx) as u32;
        queue.write_buffer(&self.buffer, offset.into(), bytemuck::cast_slice(data));
        Ok((node_idx, offset..offset + (data.len() as u32)))
    }
}

pub(super) trait GPUAllocator<T: Pod> {
    type UploadJob<'a>;
    type AllocationError: Error;

    fn upload<'a>(
        &mut self,
        job: Self::UploadJob<'a>,
        queue: &wgpu::Queue,
    ) -> Result<(), Self::AllocationError>;

    fn resolve(&self, handle: &GPUAllocationHandle) -> (Range<u32>, &wgpu::Buffer);

    fn new(device: &wgpu::Device) -> Self;
}

pub(super) trait GPUInstanceAllocator<T: Pod> {
    type AllocationError: Error;

    fn upload<'a>(
        &mut self,
        job: InstanceUploadJob<'a, T>,
        queue: &wgpu::Queue,
    ) -> Result<u32, Self::AllocationError>;

    fn resolve(&self, handle: &InstanceHandle) -> u32;

    fn bind_group(&self) -> &wgpu::BindGroup;

    fn new(device: &wgpu::Device) -> Self;
    fn register_shared_lt_binding(
        &mut self,
        donor: &InstanceHandle,
        new_handle: &InstanceHandle,
    ) -> Result<u32, Self::AllocationError>;

    fn resolve_buffer(&self, instance_handle: &InstanceHandle) -> &wgpu::Buffer;
}
#[derive(Debug)]
pub enum FreeListAllocError {
    NoRoomLeft(u32, u32),
}

impl Error for FreeListAllocError {}
impl Display for FreeListAllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRoomLeft(size, used) => f.write_str(
                format!(
                    "Not enough room to fit data of size {}. Available: {}",
                    size, used,
                )
                .as_str(),
            ),
        }
    }
}

#[derive(Debug)]
pub enum VertexArenaError {
    DataTooLarge(u32, String),
    FreeListError(FreeListAllocError),
    HandleNotFound(InstanceHandle),
    MaxAllocationReached,
}

impl From<FreeListAllocError> for VertexArenaError {
    fn from(value: FreeListAllocError) -> Self {
        Self::FreeListError(value)
    }
}

impl Display for VertexArenaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataTooLarge(size, label) => f.write_str(
                format!(
                    "cannot allocate into {} mesh of size {}, which exceeds chunk size: {}",
                    label, size, CHUNK_SIZE
                )
                .as_str(),
            ),
            Self::FreeListError(err) => Display::fmt(&err, f),
            Self::MaxAllocationReached => f.write_str(
                "All Chunks are allocated, and there is no room in any of them for this upload",
            ),
            Self::HandleNotFound(handle) => write!(
                f,
                "you probably tried to resolve shared instance data from an instance arena, but the handle {:?} was not found to be within the arena's alloc table",
                handle
            ),
        }
    }
}

impl Error for VertexArenaError {}

pub(super) struct UploadMeshJob<'frame, V: ModelVertex> {
    pub verts: &'frame [u8],
    pub(super) global_alloc_id: u32,
    _t: PhantomData<V>,
}

impl<'frame, V: ModelVertex> UploadMeshJob<'frame, V> {
    pub(super) fn new(verts: &'frame [u8], alloc_id: u32) -> Self {
        Self {
            verts,
            global_alloc_id: alloc_id,
            _t: PhantomData,
        }
    }
}

pub(super) struct UploadIndexJob<'frame> {
    pub indices: &'frame [u8],
    pub(super) global_alloc_id: u32,
}
