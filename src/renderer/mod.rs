use core::panic;
use std::fmt::Debug;
use std::hash::Hash;
use std::range::Range;
use std::{collections::HashMap, error::Error, fmt::Display, marker::PhantomData};

use bytemuck::Pod;

use crate::common::instance::InstanceHandle;
use crate::renderer::RenderConstant::DataRef;
use crate::renderer::gpu_allocator::gpu_arena::GPUArena;
use crate::renderer::gpu_allocator::{GPUUploadJob, GPUUploadResult};
use crate::world::InstanceResidency;
use crate::{
    renderer::gpu_allocator::{GPUChunk, UploadMeshJob, VertexArenaError},
    util::types::{GlobalTransform, ModelVertex},
    world::RenderKey,
};

mod bind_groups;
mod depth_tex;
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
    fn insert_default(gpu_arena: &mut GPUArena<Self>, queue: &wgpu::Queue, device: &wgpu::Device);
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

    pub fn reset(&mut self, size: usize, record_len: usize) {
        use cgmath::SquareMatrix;
        if self.global_transforms.len() < record_len {
            self.global_transforms
                .resize(record_len, cgmath::Matrix4::<f32>::identity().into());
        }
        self.draw_packet.reset(size, record_len);
    }

    pub fn count_sort(
        &mut self,
        handles: &[InstanceHandle],
        record_idxs: &[InstanceResidency],
        sparse_entity_group: &[usize],
        positions: &Vec<GlobalTransform>,
    ) {
        self.draw_packet
            .count_sort(handles, record_idxs, sparse_entity_group);

        // finally, for each record index on the gpu, and each corresponding index handle,
        // create an indirection list where indirection_list[i] = the gpu record slot
        // and i = instance idx
        // this effectively is a translation from instance_idx -> instance record idx
        // also update global_transforms such that global_transforms[i] = the transform instance i
        for (i, (residency, handle)) in record_idxs.iter().zip(handles).enumerate() {
            let group_id = sparse_entity_group[handle.entity_handle.0 as usize] as u64;
            let res = residency.bind_key as u64;
            let key = ((group_id << 32) | res) as u64;
            let bucket_idx = self.draw_packet.bucket_map[&key];
            self.draw_packet.indirection_list[self.draw_packet.cursors[bucket_idx] as usize] =
                residency.record_index;
            self.global_transforms[self.draw_packet.cursors[bucket_idx] as usize] = positions[i];
            self.draw_packet.cursors[bucket_idx] += 1;
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrawBucket {
    pub group_idx: usize,
    pub start: u32,
    pub count: u32,
}

#[derive(Debug, Default)]
pub(crate) struct DrawPacket {
    pub(crate) pnu: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
    pub(crate) pnujw: HashMap<GPUAllocationHandle, Vec<DrawItem>>,
    counts: Vec<usize>,
    cursors: Vec<u32>,
    bucket_map: HashMap<u64, usize>,
    pub(crate) draw_buckets: Vec<DrawBucket>,

    pub(crate) indirection_list: Vec<u32>,
}

impl DrawPacket {
    pub fn count_sort(
        &mut self,
        handles: &[InstanceHandle],
        residencies: &[InstanceResidency],
        sparse_entity_group: &[usize],
    ) {
        // build entity_count list, where entity_count[i] = number of entities
        // and i = render group index + instance bind key
        for (handle, res) in handles.iter().zip(residencies) {
            if res.is_pending() {
                continue;
            }
            let group_id = sparse_entity_group[handle.entity_handle.0 as usize] as u64;
            let bind_key = res.bind_key as u64;
            let bucket_key = ((group_id << 32) | bind_key) as u64;
            if let Some(bucket_idx) = self.bucket_map.get(&bucket_key) {
                self.counts[*bucket_idx] += 1;
            } else {
                self.counts[self.draw_buckets.len()] += 1;
                self.bucket_map.insert(bucket_key, self.draw_buckets.len());
                self.draw_buckets.push(DrawBucket {
                    group_idx: group_id as usize,
                    start: 0,
                    count: 0,
                });
            }
        }
        // build instance_ranges, where instance_ranges[i] = the GPU shader instance idx range
        // and i = group + bind key index
        // cusors keeps track of the first instance of the entity associated with render_groups[i]
        let mut sum = 0;
        for (bucket_idx, count) in self.counts.iter_mut().enumerate() {
            self.draw_buckets[bucket_idx].start = sum;
            self.draw_buckets[bucket_idx].count = *count as u32;
            self.cursors[bucket_idx] = sum;
            sum += *count as u32;
            *count = 0;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pnu.is_empty() && self.pnujw.is_empty()
    }

    pub fn reset(&mut self, group_bindings_count: usize, record_len: usize) {
        self.pnu.clear();
        self.pnujw.clear();
        // TODO: this isnt right unless group -> binding is 1:1
        self.counts.resize(group_bindings_count, usize::MIN);
        self.cursors.resize(group_bindings_count, u32::MAX);
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

#[derive(Clone, Debug)]
pub struct InstanceBindKey {
    lt: u16,
    jt: u16,
}

impl InstanceBindKey {
    pub fn as_u32(self) -> u32 {
        (((self.lt as u32) << 16) | self.jt as u32) as u32
    }

    pub fn from_u32(val: u32) -> Self {
        todo!()
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
        binding_key: InstanceBindKey,
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

#[derive(Clone, Copy, Debug)]
pub(crate) enum TexDim {
    Dim1,
    Dim64,
    Dim128,
    Dim256,
    Dim1024,
}

impl TexDim {
    pub(in super::renderer) fn as_u32(self) -> u32 {
        match self {
            TexDim::Dim1 => 1,
            TexDim::Dim64 => 64,
            TexDim::Dim128 => 128,
            TexDim::Dim256 => 256,
            TexDim::Dim1024 => 1024,
        }
    }
    pub(crate) fn from_u32(val: u32) -> Self {
        match val {
            1 => Self::Dim1,
            64 => Self::Dim64,
            128 => Self::Dim128,
            256 => Self::Dim256,
            1024 => Self::Dim1024,
            _ => todo!("tex dim not implemented for {val}"),
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
    TexDim(TexDim),
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
    TextureUpload,
    TexureDefault,
    TextureAcquire,
    MaterialUpload,
    EmitAssetUpload,
    EmitEntitySpawn,
    DespawnInstance,
    DespawnAsset,
    Pop,
    Swap,
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
    TextureSlot(u32),
}

impl StackValue {
    fn as_alloc(self) -> GPUAllocationHandle {
        match self {
            StackValue::Alloc(a) => a,
            StackValue::Key(a) => GPUAllocationHandle {
                global_allocation_id: a as u32,
            },
            _ => panic!("expected an alloc key, got {self:?}"),
        }
    }
    fn as_texture_slot(self) -> u32 {
        match self {
            StackValue::TextureSlot(val) => val,
            _ => panic!("expected texuture slot, got {self:?}"),
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
    pub(crate) material: Option<u32>,
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
