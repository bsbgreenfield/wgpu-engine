use std::{collections::HashMap, error::Error, fmt::Display, ops::Range};

#[cfg(test)]
use crate::world::scene::SceneLoadLevel;
use crate::{
    app::{
        GPUUploadJob,
        renderer::gpu_allocator::{
            GPUAllocator, UploadMeshJob, VertexArenaError, vertex_arena::GPUArena,
        },
    },
    asset_manager_new::AssetHandle,
    util::types::{LocalTransform, Mat4F32, ModelVertex, PNUJWVertex, PNUVertex},
    world::{
        entity_manager::{EntityHandle, Renderables},
        instance_manager::InstanceHandle,
        world::{DrawSet, RenderView},
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
pub struct LocalTransformUploadJob<'frame> {
    pub(super) local_transforms: &'frame [LocalTransform],
    pub(super) global_alloc_id: u32,
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
    UploadJob(GPUUploadJob<'frame>),
    Renderables(Renderables),
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

    fn get_arena(&self) -> &GPUArena<V>;
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
}

#[derive(Hash, PartialEq, PartialOrd, Eq)]
pub struct BufferChunks {
    index: Option<usize>,
    vertex: usize,
}

struct DrawList {
    chunks: BufferChunks,
    items: Vec<DrawItem>,
}

pub struct AllocationCache {
    chunks_index: usize,
    vertex_range: Range<u32>,
    index_range: Option<Range<u32>>,
    lt_offset: u32,
}

#[derive(Debug, Default)]
pub struct DrawPacket {
    pnu: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
    pnujw: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
}

//#[derive(Default)]
//pub struct DrawPacket {
//    pnu_draw_len: u32,
//    pnujw_draw_len: u32,
//    pub pnu: Vec<DrawList>,
//    pub pnu_cache: HashMap<u32, AllocationCache>,
//    pub pnujw: Vec<DrawList>,
//    pub pnujw_cache: HashMap<u32, AllocationCache>,
//}

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

//pub struct DrawPacket {
//    pnu: HashMap<BufferChunks, Vec<DrawItem>>,
//    pnujw: HashMap<BufferChunks, Vec<DrawItem>>,
//}
//impl DrawPacket {
//    pub fn new() -> Self {
//        Self {
//            pnu: HashMap::new(),
//            pnujw: HashMap::new(),
//        }
//    }
//    pub fn clear(&mut self) {
//        self.pnu.clear();
//        self.pnujw.clear();
//    }
//
//    pub fn is_empty(&self) -> bool {
//        self.pnu.is_empty() && self.pnujw.is_empty()
//    }
//}
//#[cfg(test)]
//impl DrawPacketNew {
//    pub fn get_pnu(&self) -> &HashMap<BufferChunks, Vec<DrawItem>> {
//        &self.pnu
//    }
//
//    pub fn get_pnujw(&self) -> &HashMap<BufferChunks, Vec<DrawItem>> {
//        &self.pnujw
//    }
//}
