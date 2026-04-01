use std::{error::Error, fmt::Display, ops::Range};

use bytemuck::Pod;

use crate::{
    asset_manager::{AssetHandle, LoadedAsset},
    util::types::Mat4F32,
    world::{
        components::MeshCollectionComponent,
        entity_manager::{EntityHandle, Renderables},
        instance_manager::InstanceHandle,
    },
};

mod free_list;
mod pipeline;
pub mod renderer;
mod vertex_arena;
mod vm;

static CHUNK_SIZE: u32 = 1024 * 4;

pub enum RenderUpdateDelta {
    AssetGPULoaded(GPUAllocationHandle),
    EntityGPULoaded(EntityHandle),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GPUAllocationHandle {
    global_allocation_id: u32,
    pub asset_handle: AssetHandle,
}

//#[derive(Hash, PartialEq, PartialOrd, Eq, Debug, Clone, Copy)]
//struct AllocationHandle<T> {
//    pub(super) global_alloc_id: u32,
//    pipeline_alloc_id: u32,
//    _t: PhantomData<T>,
//}

#[derive(Clone, Copy)]
pub(super) enum Instruction {
    Op(Operations),
    Byte(u8),
    ConstIdx(u8),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operations {
    AddAsset,
    AddEntity,
    MoveEntity,
    SpawnEntityInstance,
}

pub enum VMValue<'frame> {
    Transform(Mat4F32),
    LoadedAsset(&'frame LoadedAsset),
    MeshCollectionComponent(&'frame MeshCollectionComponent),
    Renderables(Renderables<'frame>),
    InstanceHandle(InstanceHandle),
}

trait GPUAllocator<T: Pod> {
    type UploadJob<'a>;
    type AllocationError: Error;

    fn upload<'a>(
        &mut self,
        job: Self::UploadJob<'a>,
        queue: &wgpu::Queue,
    ) -> Result<(), Self::AllocationError>;

    fn resolve(
        &self,
        handle: &GPUAllocationHandle,
    ) -> (Range<u32>, &wgpu::Buffer, Option<&wgpu::BindGroup>);

    fn chunk_id(&self, handle: &GPUAllocationHandle) -> usize;

    fn buffer_from_chunk_id(&self, chunk_id: usize) -> &wgpu::Buffer;

    fn new(device: &wgpu::Device) -> Self;
}

#[derive(Debug)]
pub(super) enum FreeListAllocError {
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
pub(super) enum VertexArenaError {
    DataTooLarge(u32),
    FreeListError(FreeListAllocError),
}

impl From<FreeListAllocError> for VertexArenaError {
    fn from(value: FreeListAllocError) -> Self {
        Self::FreeListError(value)
    }
}

impl Display for VertexArenaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::DataTooLarge(size) => f.write_str(
                format!(
                    "cannot allocate mesh of size {}, which exceeds chunk size: {}",
                    size, CHUNK_SIZE
                )
                .as_str(),
            ),
            Self::FreeListError(err) => err.fmt(f),
        }
    }
}

impl Error for VertexArenaError {}

#[derive(Debug)]
pub enum RenderUpdateError {
    MeshUploadFailed(String),
    LocalTransformUpdateFailed,
}

impl From<VertexArenaError> for RenderUpdateError {
    fn from(value: VertexArenaError) -> Self {
        match value {
            VertexArenaError::DataTooLarge(size) => Self::MeshUploadFailed(format!(
                "upload failed because data of size {size} was too large"
            )),
            VertexArenaError::FreeListError(e) => {
                Self::MeshUploadFailed(format!("Upload failed due to allocation error {}", e))
            }
        }
    }
}

#[derive(Debug)]
pub enum RenderError {
    SurfaceError(wgpu::SurfaceError),
}

impl From<wgpu::SurfaceError> for RenderError {
    fn from(value: wgpu::SurfaceError) -> Self {
        Self::SurfaceError(value)
    }
}

impl Display for RenderUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MeshUploadFailed(desc) => desc.fmt(f),
            Self::LocalTransformUpdateFailed => {
                f.write_str("Local Transform data could not be uploaded")
            }
        }
    }
}

impl Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SurfaceError(e) => e.fmt(f),
        }
    }
}

impl Error for RenderUpdateError {}
