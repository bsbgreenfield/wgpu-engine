use std::fmt::Debug;
use std::hash::Hash;
use std::range::Range;
use std::{collections::HashMap, error::Error, fmt::Display, marker::PhantomData};

use bytemuck::Pod;

use crate::common::instance::InstanceHandle;
use crate::renderer::RenderConstant::DataRef;
use crate::renderer::gpu_allocator::gpu_arena::GPUArena;
use crate::renderer::gpu_allocator::{GPUUploadJob, GPUUploadResult};
use crate::{
    renderer::gpu_allocator::{GPUChunk, UploadMeshJob, VertexArenaError},
    util::types::{GlobalTransform, ModelVertex},
    world::RenderKey,
};

mod bind_groups;
mod gpu_allocator;
mod pipeline;
pub(crate) mod renderer;
mod vm;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct PrototypeHandle(u32);

impl PrototypeHandle {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}
impl RenderKey for PrototypeHandle {
    fn as_key(&self) -> u64 {
        self.0 as u64
    }

    fn from_key(key: u64) -> Self {
        Self(key as u32)
    }
}

trait GPUUploadable: Debug + bytemuck::Pod {
    type GPUHandle: Debug + Clone + Hash + Eq;
    type UploadJob<'a>: GPUUploadJob<GPUHandle = Self::GPUHandle>;
    const LABEL: &'static str;
    const USAGE: wgpu::BufferUsages;
    const CHUNK_SIZE: u32;
    const MIN_ALLOC_SIZE: u32;
    const SIZE: usize = size_of::<Self>();
    fn arena_label() -> String;
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        GPUChunk::new(device, Self::CHUNK_SIZE, Self::LABEL, Self::USAGE)
    }
    fn upload(
        arena: &mut GPUArena<Self>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult;
}

pub struct RenderPacket {
    pub(crate) global_transforms: Vec<GlobalTransform>,
    pub(crate) draw_packet: DrawPacket,
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

    pub fn count_sort(
        &mut self,
        handles: &[InstanceHandle],
        record_idxs: &[u32],
        sparse_entity_group: &[usize],
        positions: &Vec<GlobalTransform>,
    ) {
        self.draw_packet.count_sort(handles, sparse_entity_group);

        // finally, for each record index on the gpu, and each corresponding index handle,
        // create an indirection list where indirection_list[i] = the gpu record slot
        // and i = instance idx
        // this effectively is a translation from instance_idx -> instance record idx
        // also update global_transforms such that global_transforms[i] = the transform instance i
        for (i, (record_index, handle)) in record_idxs.iter().zip(handles).enumerate() {
            let group_id = sparse_entity_group[handle.entity_handle.0 as usize];
            self.draw_packet.indirection_list[self.draw_packet.cursors[group_id] as usize] =
                *record_index;
            self.global_transforms[self.draw_packet.cursors[group_id] as usize] = positions[i];
            self.draw_packet.cursors[group_id] += 1;
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct DrawPacket {
    pub(crate) pnu: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
    pub(crate) pnujw: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
    entity_count: Vec<usize>,
    cursors: Vec<u32>,
    pub(crate) instance_ranges: Vec<Range<u32>>,
    pub(crate) indirection_list: Vec<u32>,
}

impl DrawPacket {
    pub fn count_sort(&mut self, handles: &[InstanceHandle], sparse_entity_group: &[usize]) {
        // build entity_count list, where entity_count[i] = number of entities
        // and i = render group index
        for handle in handles {
            let group_id = sparse_entity_group[handle.entity_handle.0 as usize];
            self.entity_count[group_id] += 1;
        }
        // build instance_ranges, where instance_ranges[i] = the GPU shader instance idx range
        // and i = render group idx
        // cusors keeps track of the first instance of the entity associated with render_groups[i]
        let mut sum = 0;
        for (group_id, count) in self.entity_count.iter_mut().enumerate() {
            self.instance_ranges[group_id] = Range::from(sum..(sum + *count as u32));
            self.cursors[group_id] = sum;
            sum += *count as u32;
            *count = 0;
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
    pub(crate) fn get_pnu(&self) -> &HashMap<GPUAllocationHandle, Vec<DrawItem>> {
        &self.pnu
    }

    #[cfg(test)]
    pub(crate) fn get_pnujw(&self) -> &HashMap<GPUAllocationHandle, Vec<DrawItem>> {
        &self.pnujw
    }
}
#[derive(Debug, Clone)]
pub(crate) enum RenderUpdateDelta {
    AssetUnloaded {
        key: u64,
        alloc_handle: GPUAllocationHandle,
    },
    AssetGPULoaded {
        key: u64,
        alloc_handle: GPUAllocationHandle,
    },
    EntitySpawned {
        instance_key: u64,
        gpu_instance_handle: GPUInstanceHandle,
        record_offset: u32,
    },
    InstanceDespawn(GPUInstanceHandle),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GPUInstanceHandle {
    pub(crate) prototype: PrototypeHandle,
    pub(crate) instance_id: u32,
}

#[cfg(test)]
impl GPUInstanceHandle {
    pub fn prototype_id(&self) -> u32 {
        self.prototype.0
    }
    pub fn instance_id(&self) -> u32 {
        self.instance_id
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct GPUAllocationHandle {
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
    pub(crate) fn mock(global_allocation_id: u32) -> Self {
        Self {
            global_allocation_id,
        }
    }
}

// pub(crate) only because it's `GPUUploadable::UploadJob` for the `StorageData` blanket impl,
// and `GPUUploadable` itself must be pub(crate) (`ModelVertex` in util/types.rs requires it).
#[derive(Debug)]
pub(crate) struct InstanceUploadJob<'a, T: Pod> {
    data: &'a [u8],
    gpu_instance_handle: GPUInstanceHandle,
    _t: PhantomData<T>,
}

impl<'a, T: Pod> InstanceUploadJob<'a, T> {
    fn new(data: &'a [u8], gpu_instance_handle: GPUInstanceHandle) -> Self {
        Self {
            data,
            gpu_instance_handle,
            _t: PhantomData,
        }
    }
}

#[allow(unused)]
#[derive(Clone, Copy, Debug)]
pub(crate) enum Instruction {
    Op(Operations),
    Byte(u8),
    ConstIdx(u8),
    WideIdx(u8),
    Buffer(BufferType),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum BufferType {
    LocalTransform,
    JointTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operations {
    CreatePrototype,
    AddAsset,
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
pub(crate) enum RenderConstant<'frame> {
    DataRef(&'frame [u8]),
    Key(u64),
}

#[derive(Debug)]
enum StackValue {
    Key(u64),
    Alloc(GPUAllocationHandle),
    Instance(GPUInstanceHandle),
    Offset(u32),
}

impl StackValue {
    fn as_alloc(self) -> GPUAllocationHandle {
        match self {
            StackValue::Alloc(a) => a,
            _ => panic!("expected an alloc key, got {self:?}"),
        }
    }

    fn as_instance_handle(self) -> GPUInstanceHandle {
        match self {
            StackValue::Instance(i) => i,
            _ => panic!("expected gpu instance handle, got {self:?}"),
        }
    }
    fn as_offset(self) -> u32 {
        match self {
            StackValue::Offset(o) => o,
            _ => panic!("expected offset, got {self:?}"),
        }
    }

    fn as_raw_key<'a>(self) -> u64 {
        match self {
            StackValue::Key(key) => key,
            _ => panic!("expected key, got {self:?}"),
        }
    }
}

impl From<RenderConstant<'_>> for StackValue {
    fn from(value: RenderConstant<'_>) -> Self {
        match value {
            DataRef(_) => panic!("cannot push binary data onto the stack"),
            RenderConstant::Key(key) => StackValue::Key(key),
        }
    }
}

impl<'frame> Clone for RenderConstant<'frame> {
    fn clone(&self) -> Self {
        match self {
            Self::Key(key) => Self::Key(*key),
            Self::DataRef(_) => panic!("cannot clone ref data (maybe make it an arc)"),
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
    SurfaceError(wgpu::CreateSurfaceError),
    BadSurfaceTexture,
}

impl From<wgpu::CreateSurfaceError> for RenderError {
    fn from(value: wgpu::CreateSurfaceError) -> Self {
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
            Self::SurfaceError(e) => std::fmt::Display::fmt(&e, f),
            Self::BadSurfaceTexture => write!(f, "Sub Optimal or invalid surface"),
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
pub(crate) enum RenderCategory {
    OpaqueStatic,
    OpaqueSkinned,
}

#[derive(Debug)]
pub(crate) struct DrawItem {
    pub(crate) lt_idx: u32,
    pub(crate) joint_offset: Option<u32>,
    pub(crate) instances: Range<u32>,
    pub(crate) primitives: Range<u32>,
    pub(crate) indices: Option<Range<u32>>,
}

#[cfg(test)]
impl DrawItem {
    pub(crate) fn get_lt_idx(&self) -> u32 {
        self.lt_idx
    }

    pub(crate) fn get_instances(&self) -> Range<u32> {
        self.instances.clone()
    }
    pub(crate) fn get_primitives(&self) -> Range<u32> {
        self.primitives.clone()
    }
    pub(crate) fn get_indices(&self) -> Option<Range<u32>> {
        self.indices.clone()
    }
}

use bitflags::bitflags;
bitflags! {
    pub(crate) struct GPUBindings: u8 {
        const LOCAL_TRANSFORM = 0b01;
        const JOINT_TRANSFORM = 0b10;
    }
}

trait StorageData: bytemuck::Pod + std::fmt::Debug + Sized {
    const LABEL: &'static str;
    const CHUNK_SIZE: u32;
}
