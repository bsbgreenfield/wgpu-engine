use std::{collections::HashMap, error::Error, fmt::Display, ops::Range};

use bytemuck::Pod;

use crate::{
    app::{
        GPUAssetUploadJob,
        renderer::gpu_allocator::{UploadMeshJob, VertexArenaError},
    },
    asset_manager_new::AssetHandle,
    util::types::{Mat4F32, ModelVertex},
    world::{
        entity_manager::{EntityHandle, LocalTransformData, Renderables},
        instance_manager::{InstanceGPUBindings, InstanceHandle},
        world::InstanceUploadData,
    },
};

mod gpu_allocator;
mod pipeline;
pub mod renderer;
mod vm;

#[derive(Debug)]
pub enum RenderUpdateDelta {
    AssetGPULoaded(AssetHandle, GPUAllocationHandle),
    EntityGPULoaded(EntityHandle),
    EntitySpawned((InstanceHandle, InstanceGPUBindings)),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GPUAllocationHandle {
    global_allocation_id: u32,
}

#[cfg(test)]
impl GPUAllocationHandle {
    pub fn mock(global_allocation_id: u32) -> Self {
        Self {
            global_allocation_id,
        }
    }
}

#[derive(Debug)]
pub struct InstanceUploadJob<'a, T: Pod> {
    pub data: &'a [T],
    pub instance_handle: InstanceHandle,
}

impl<'a, T: Pod> InstanceUploadJob<'a, T> {
    pub fn new(data: &'a [T], instance_handle: InstanceHandle) -> Self {
        Self {
            data,
            instance_handle,
        }
    }
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
    InstanceDataUpload(&'frame InstanceUploadData),
    Transform(Mat4F32),
    UploadJob(GPUAssetUploadJob<'frame>),
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
impl Error for RenderError {}

trait VertexArenaSelector<V: ModelVertex> {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<V>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError>;
}
pub enum RenderCategory {
    OpaqueStatic,
    OpaqueSkinned,
}

#[derive(Debug)]
pub struct DrawItem {
    lt_idx: u32,
    instances: Range<u32>,
    primitives: Range<u32>,
    indices: Option<Range<u32>>,
}

#[cfg(test)]
impl DrawItem {
    pub fn get_lt_idx(&self) -> u32 {
        self.lt_idx
    }

    pub fn get_instances(&self) -> Range<u32> {
        self.instances.clone()
    }
    pub fn get_primitives(&self) -> Range<u32> {
        self.primitives.clone()
    }
    pub fn get_indices(&self) -> Option<Range<u32>> {
        self.indices.clone()
    }
}

#[derive(Hash, PartialEq, PartialOrd, Eq)]
pub struct BufferChunks {
    index: Option<usize>,
    vertex: usize,
}

#[derive(Debug, Default)]
pub struct DrawPacket {
    pnu: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
    pnujw: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
}

impl DrawPacket {
    pub fn is_empty(&self) -> bool {
        self.pnu.is_empty() && self.pnujw.is_empty()
    }

    pub fn clear(&mut self) {
        self.pnu.clear();
        self.pnujw.clear();
    }

    #[cfg(test)]
    pub fn get_pnu(&self) -> &HashMap<GPUAllocationHandle, Vec<DrawItem>> {
        &self.pnu
    }

    pub fn get_pnujw(&self) -> &HashMap<GPUAllocationHandle, Vec<DrawItem>> {
        &self.pnujw
    }
}
