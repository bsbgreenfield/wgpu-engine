use std::{collections::HashMap, error::Error, fmt::Display, ops::Range};

use crate::{
    app::renderer::gpu_allocator::{
        GPUAllocator, UploadMeshJob, VertexArenaError, vertex_arena::GPUArena,
    },
    asset_manager::{AssetHandle, LoadedAsset},
    util::types::{Mat4F32, ModelVertex, PNUJWVertex, PNUVertex},
    world::{
        components::MeshCollectionComponent,
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

trait DrawListBuilder<V: ModelVertex> {
    fn write_list(
        view: &RenderView,
        arena: &GPUArena<V>,
        draw_list: &mut Vec<DrawItem>,
        instance_idx: u32,
        lt_offset: u32,
    );
}

impl DrawListBuilder<PNUVertex> for DrawPacket {
    fn write_list(
        view: &RenderView,
        arena: &GPUArena<PNUVertex>,
        draw_list: &mut Vec<DrawItem>,
        instance_idx: u32,
        lt_offset: u32,
    ) {
        let pnu_draws = view.pnu_draws.as_ref().unwrap();
        for (i, mesh_id) in pnu_draws.mesh_ids.iter().enumerate() {
            let (alloc_range, _, _) = arena.resolve(&view.gpu_handle);

            let prim_range = DrawSet::within(&pnu_draws.primtitive_ranges[i], &alloc_range);

            let indices = pnu_draws
                .index_ranges
                .as_ref()
                .map(|index_ranges| DrawSet::within(&index_ranges[i], &alloc_range));
            draw_list.push(DrawItem {
                lt_idx: lt_offset + mesh_id,
                instances: instance_idx..instance_idx + 1,
                primitives: prim_range,
                indices: indices,
            });
        }
    }
}

impl DrawListBuilder<PNUJWVertex> for DrawPacket {
    fn write_list(
        view: &RenderView,
        arena: &GPUArena<PNUJWVertex>,
        draw_list: &mut Vec<DrawItem>,
        instance_idx: u32,
        lt_offset: u32,
    ) {
        let pnujw_draws = view.pnujw_draws.as_ref().unwrap();
        for (i, mesh_id) in pnujw_draws.mesh_ids.iter().enumerate() {
            let (alloc_range, _, _) = arena.resolve(&view.gpu_handle);
            let prim_range = DrawSet::within(&pnujw_draws.primtitive_ranges[i], &alloc_range);
            let indices = pnujw_draws
                .index_ranges
                .as_ref()
                .map(|index_ranges| DrawSet::within(&index_ranges[i], &alloc_range));
            draw_list.push(DrawItem {
                lt_idx: lt_offset + mesh_id,
                instances: instance_idx..instance_idx + 1,
                primitives: prim_range,
                indices,
            });
        }
    }
}

#[derive(Hash, PartialEq, PartialOrd, Eq)]
pub struct BufferChunks {
    index: Option<usize>,
    vertex: usize,
}
pub struct DrawPacket {
    pnu: HashMap<BufferChunks, Vec<DrawItem>>,
    pnujw: HashMap<BufferChunks, Vec<DrawItem>>,
}

#[cfg(test)]
impl DrawPacket {
    pub fn get_pnu(&self) -> &HashMap<BufferChunks, Vec<DrawItem>> {
        &self.pnu
    }

    pub fn get_pnujw(&self) -> &HashMap<BufferChunks, Vec<DrawItem>> {
        &self.pnujw
    }
}
