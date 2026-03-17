use std::{error::Error, ops::Range};

use bytemuck::Pod;

use crate::{
    asset_manager::asset_manager::{AssetHandle, LoadedAsset},
    util::types::Mat4F32,
    world::{
        components::MeshCollectionComponent,
        entity_manager::{EntityHandle, Renderables},
        world::RenderGroup,
    },
};

mod free_list;
mod pipeline;
pub mod renderer_new;
mod vertex_arena;
mod vm;

static CHUNK_SIZE: u32 = 1024 * 4;

pub enum RenderUpdateDeltaNew {
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
}

pub enum VMValue<'frame> {
    Transform(Mat4F32),
    LoadedAsset(&'frame LoadedAsset),
    RenderGroup(RenderGroup),
    MeshCollectionComponent(&'frame MeshCollectionComponent),
    EntityHandle(&'frame EntityHandle),
    Renderables(Renderables<'frame>),
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

    fn new(device: &wgpu::Device) -> Self;
}
