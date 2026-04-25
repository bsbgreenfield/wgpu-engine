use std::{fmt::Display, ops::Range};

use bytemuck::Pod;
use std::error::Error;
use wgpu::wgc::device;

use crate::{
    app::renderer::GPUAllocationHandle,
    util::types::{LocalTransform, ModelVertex, VIndex},
    world::instance_manager::InstanceHandle,
};

mod free_list;
pub(super) mod instance_arena;
pub(super) mod vertex_arena;

static CHUNK_SIZE: u32 = 4_194_304;
pub(super) trait GPUAllocator<T: Pod> {
    type UploadJob<'a>;
    type AllocationError: Error;

    fn upload<'a>(
        &mut self,
        job: Self::UploadJob<'a>,
        queue: &wgpu::Queue,
    ) -> Result<(), Self::AllocationError>;

    fn resolve(&self, handle: &GPUAllocationHandle) -> (Range<u32>, &wgpu::Buffer);

    fn chunk_id(&self, handle: &GPUAllocationHandle) -> usize;

    fn buffer_from_chunk_id(&self, chunk_id: usize) -> &wgpu::Buffer;

    fn new(device: &wgpu::Device) -> Self;
}

pub(super) trait GPUInstanceAllocator<T: Pod> {
    type UploadJob<'a>;
    type AllocationError: Error;

    fn upload<'a>(
        self,
        job: Self::UploadJob<'a>,
        queue: &wgpu::Queue,
    ) -> Result<(), Self::AllocationError>;

    fn resolve(&self, handle: &InstanceHandle) -> (u32, &wgpu::BindGroup);

    fn new(device: &wgpu::Device) -> Self;
}
pub struct LocalTransformUploadJob<'frame> {
    pub(super) local_transforms: &'frame [LocalTransform],
    pub(super) global_alloc_id: u32,
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
    MaxAllocationReached,
}

impl From<FreeListAllocError> for VertexArenaError {
    fn from(value: FreeListAllocError) -> Self {
        Self::FreeListError(value)
    }
}

impl Display for VertexArenaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::DataTooLarge(size, label) => f.write_str(
                format!(
                    "cannot allocate into {} mesh of size {}, which exceeds chunk size: {}",
                    label, size, CHUNK_SIZE
                )
                .as_str(),
            ),
            Self::FreeListError(err) => err.fmt(f),
            Self::MaxAllocationReached => f.write_str(
                "All Chunks are allocated, and there is no room in any of them for this upload",
            ),
        }
    }
}

impl Error for VertexArenaError {}

pub(super) struct UploadMeshJob<'frame, V: ModelVertex> {
    pub verts: &'frame [V],
    pub(super) global_alloc_id: u32,
}

pub(super) struct UploadIndexJob<'frame> {
    pub indices: &'frame [VIndex],
    pub(super) global_alloc_id: u32,
}
