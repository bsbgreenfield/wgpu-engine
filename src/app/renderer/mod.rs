use std::{error::Error, fmt::Display};

use crate::{
    app::renderer::gpu_allocator::{UploadMeshJob, VertexArenaError, vertex_arena::GPUArena},
    asset_manager::{AssetHandle, LoadedAsset},
    util::types::{Mat4F32, ModelVertex},
    world::{
        components::MeshCollectionComponent,
        entity_manager::{EntityHandle, Renderables},
        instance_manager::InstanceHandle,
    },
};

mod gpu_allocator;
mod pipeline;
pub mod renderer;
mod vm;

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

#[allow(unused)]
#[derive(Clone, Copy, Debug)]
pub enum Instruction {
    Op(Operations),
    Byte(u8),
    ConstIdx(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operations {
    AddAsset,
    AddEntity,
    MoveEntity,
    SpawnEntityInstance,
}

#[derive(Debug)]
pub enum VMValue<'frame> {
    Transform(Mat4F32),
    LoadedAsset(&'frame LoadedAsset),
    MeshCollectionComponent(&'frame MeshCollectionComponent),
    Renderables(Renderables<'frame>),
    InstanceHandle(InstanceHandle),
}

#[derive(Debug)]
pub enum RenderUpdateError {
    GpuUploadFailure(VertexArenaError),
}

impl From<VertexArenaError> for RenderUpdateError {
    fn from(value: VertexArenaError) -> Self {
        match value {
            _ => Self::GpuUploadFailure(value),
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
            Self::GpuUploadFailure(err) => err.fmt(f),
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

trait VertexArenaSelector<V: ModelVertex> {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<V>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError>;

    fn get_arena(&self) -> &GPUArena<V>;
}
