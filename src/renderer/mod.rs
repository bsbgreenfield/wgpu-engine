use std::{collections::HashMap, error::Error, fmt::Display, marker::PhantomData, ops::Range};

use bytemuck::Pod;

use crate::{
    asset_manager::AssetHandle,
    renderer::{
        gpu_allocator::{GPUChunk, UploadMeshJob, VertexArenaError},
        renderer::GPUInstanceHandle,
    },
    util::types::{GlobalTransform, ModelVertex},
    world::{RenderKey, entity_manager::EntityHandle, instance_manager::InstanceHandle},
};

mod bind_groups;
mod gpu_allocator;
mod pipeline;
pub mod renderer;
mod vm;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct PrototypeHandle(pub u32);

impl RenderKey for PrototypeHandle {
    fn as_key(&self) -> u64 {
        self.0 as u64
    }

    fn from_key(key: u64) -> Self {
        Self(key as u32)
    }
}

pub struct RenderPacket {
    pub global_transforms: Vec<GlobalTransform>,
    pub draw_packet: DrawPacket,
}

impl RenderPacket {
    pub fn new() -> Self {
        Self {
            global_transforms: Vec::new(),
            draw_packet: DrawPacket::default(),
        }
    }

    pub fn reset(&mut self, group_len: usize, record_len: usize) {
        use cgmath::SquareMatrix;
        if self.global_transforms.len() < record_len {
            self.global_transforms
                .resize(record_len, cgmath::Matrix4::<f32>::identity().into());
        }
        self.draw_packet.reset(group_len, record_len);
    }
}

#[derive(Debug, Default)]
pub struct DrawPacket {
    pub pnu: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
    pub pnujw: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
    pub entity_count: Vec<usize>,
    pub cursors: Vec<u32>,
    pub instance_ranges: Vec<Range<u32>>,
    pub indirection_list: Vec<u32>,
}

impl DrawPacket {
    pub fn count_sort(
        &mut self,
        handles: &[InstanceHandle],
        record_idxs: &[u32],
        sparse_entity_group: &[usize],
    ) {
        for handle in handles {
            let group_id = sparse_entity_group[handle.entity_handle.0 as usize];
            self.entity_count[group_id] += 1;
        }
        let mut sum = 0;
        for (group_id, count) in self.entity_count.iter_mut().enumerate() {
            self.instance_ranges[group_id] = sum..(sum + *count as u32);
            self.cursors[group_id] = sum;
            sum += *count as u32;
            *count = 0;
        }
        for (record_index, handle) in record_idxs.iter().zip(handles) {
            let group_id = sparse_entity_group[handle.entity_handle.0 as usize];
            self.indirection_list[self.cursors[group_id] as usize] = *record_index;
            self.cursors[group_id] += 1;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pnu.is_empty() && self.pnujw.is_empty()
    }

    pub fn reset(&mut self, group_len: usize, record_len: usize) {
        self.pnu.clear();
        self.pnujw.clear();
        self.entity_count.resize(group_len, usize::MIN);
        self.cursors.resize(group_len, u32::MAX);
        self.instance_ranges
            .resize(group_len, Range::<u32> { start: 0, end: 0 });
        self.indirection_list.resize(record_len, u32::MAX);
    }

    #[cfg(test)]
    pub fn get_pnu(&self) -> &HashMap<GPUAllocationHandle, Vec<DrawItem>> {
        &self.pnu
    }

    pub fn get_pnujw(&self) -> &HashMap<GPUAllocationHandle, Vec<DrawItem>> {
        &self.pnujw
    }
}
#[derive(Debug)]
pub enum RenderUpdateDelta {
    AssetGPULoaded(AssetHandle, GPUAllocationHandle),
    EntityGPULoaded(EntityHandle),
    EntitySpawned {
        instance_handle: InstanceHandle,
        gpu_instance_handle: GPUInstanceHandle,
        record_offset: u32,
    },
    ProtypeCreated {
        instance_handle: InstanceHandle,
        prototype: PrototypeHandle,
    },
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GPUAllocationHandle {
    global_allocation_id: u32,
}

impl RenderKey for GPUAllocationHandle {
    fn as_key(&self) -> u64 {
        self.global_allocation_id as u64
    }
    fn from_key(key: u64) -> Self {
        Self {
            global_allocation_id: key as u32,
        }
    }
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
    pub data: &'a [u8],
    pub gpu_instance_handle: GPUInstanceHandle,
    _t: PhantomData<T>,
}

impl<'a, T: Pod> InstanceUploadJob<'a, T> {
    pub fn new(data: &'a [u8], gpu_instance_handle: GPUInstanceHandle) -> Self {
        Self {
            data,
            gpu_instance_handle,
            _t: PhantomData,
        }
    }
}

#[allow(unused)]
#[derive(Clone, Copy, Debug)]
pub enum Instruction {
    Op(Operations),
    Byte(u8),
    ConstIdx(u8),
    WideIdx(u8),
    Buffer(BufferType),
}

#[derive(Debug, Clone, Copy)]
pub enum BufferType {
    LocalTransform,
    JointTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operations {
    CreatePrototype,
    AddAsset,
    MoveEntity,
    SpawnEntityInstance,
    LocalTransformUpload,
    JointTransformUpload,
    SpawnFromPrototype,
    ShareData,
    CopyData,
    PNUUpload,
    PNUJWUpload,
    IndexUpload,
    EmitAssetUpload,
    EmitEntitySpawn,
    DespawnInstance,
    DespawnAsset,
    Pop,
    Push,
}

#[derive(Debug)]
pub enum RenderConstant<'frame> {
    DataOwned(Vec<u8>),
    DataRef(&'frame [u8]),
    Key(u64),
    Offset(u64),
}

impl<'frame> Clone for RenderConstant<'frame> {
    fn clone(&self) -> Self {
        match self {
            Self::Key(key) => Self::Key(*key),
            Self::Offset(offset) => Self::Offset(*offset),
            Self::DataRef(_) => panic!("cannot clone ref data (maybe make it an arc)"),
            Self::DataOwned(_) => panic!("cannot clone owned data"),
        }
    }
}

impl<'frame> RenderConstant<'frame> {
    fn unwrap_key(&self) -> u64 {
        match self {
            Self::Key(key) => *key,
            _ => panic!("invalid bytecode, expected key, found {:?}", self),
        }
    }

    fn unwrap_data_ref(&self) -> &[u8] {
        match self {
            Self::DataRef(data_ref) => data_ref,
            _ => panic!("invalid bytecode, expected data, found {:?}", self),
        }
    }
    fn unwrap_data_owned(&self) -> &[u8] {
        match self {
            Self::DataOwned(data) => data,
            _ => panic!("invalid bytecode, expected data, found {:?}", self),
        }
    }

    fn unwrap_offset(&self) -> u64 {
        match self {
            Self::Offset(offset) => *offset,
            _ => panic!("invalid bytecode, expected offset, found {:?}", self),
        }
    }
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
            Self::GpuUploadFailure(err) => std::fmt::Display::fmt(err, f),
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
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError>;
}
pub enum RenderCategory {
    OpaqueStatic,
    OpaqueSkinned,
}

#[derive(Debug)]
pub struct DrawItem {
    pub lt_idx: u32,
    pub joint_offset: Option<u32>,
    pub instances: Range<u32>,
    pub primitives: Range<u32>,
    pub indices: Option<Range<u32>>,
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

use bitflags::bitflags;
bitflags! {
    pub struct GPUBindings: u8 {
        const LOCAL_TRANSFORM = 0b01;
        const JOINT_TRANSFORM = 0b10;
    }
}

trait StorageData: bytemuck::Pod + std::fmt::Debug + Sized {
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self>;
}
