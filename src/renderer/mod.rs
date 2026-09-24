use core::panic;
use std::fmt::Debug;
use std::hash::Hash;
use std::range::Range;
use std::{collections::HashMap, error::Error, fmt::Display, marker::PhantomData};

use bytemuck::Pod;

use crate::renderer::RenderConstant::DataRef;
use crate::renderer::gpu_allocator::allocation_tables::AllocationTableError;
use crate::world::InstanceResidency;
use crate::{
    renderer::gpu_allocator::{UploadMeshJob, VertexArenaError},
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
pub struct PrototypeHandle(u16);

impl PrototypeHandle {
    pub fn new(id: u16) -> Self {
        Self(id)
    }
}
impl RenderKey for PrototypeHandle {
    fn as_key(&self) -> u64 {
        self.0 as u64
    }

    fn from_key(key: u64) -> Self {
        Self(key as u16)
    }
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

    pub fn reset(&mut self, record_len: usize) {
        use cgmath::SquareMatrix;
        if self.global_transforms.len() < record_len {
            self.global_transforms
                .resize(record_len, cgmath::Matrix4::<f32>::identity().into());
        }
        self.draw_packet.reset(record_len);
    }

    pub fn count_sort(
        &mut self,
        record_idxs: &[InstanceResidency],
        positions: &Vec<GlobalTransform>,
    ) {
        self.draw_packet.count_sort(record_idxs);

        // finally, for each record index on the gpu, and each corresponding index handle,
        // create an indirection list where indirection_list[i] = the gpu record slot
        // and i = instance idx
        // this effectively is a translation from instance_idx -> instance record idx
        // also update global_transforms such that global_transforms[i] = the transform instance i
        let instance_to_bucket = &self.draw_packet.instance_to_bucket;
        let cursors = &mut self.draw_packet.cursors;
        let indirection_list = &mut self.draw_packet.indirection_list;

        for (i, bucket_idx) in instance_to_bucket.iter().enumerate() {
            let bucket_idx = *bucket_idx;
            if bucket_idx == u32::MAX {
                continue;
            }
            let slot = cursors[bucket_idx as usize] as usize;
            indirection_list[slot] = record_idxs[i].record_index;
            self.global_transforms[slot] = positions[i];
            cursors[bucket_idx as usize] += 1;
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
    cursors: Vec<u32>,
    //bucket_map: HashMap<u64, usize>,
    bucket_keys: Vec<u64>,
    pub(crate) draw_buckets: Vec<DrawBucket>,
    instance_to_bucket: Vec<u32>,
    pub(crate) indirection_list: Vec<u32>,
}

impl DrawPacket {
    // TODO: bucket keys is a linear search through the bucket keys vec
    // to find bucket idx from bucket_keys<Key>
    // if the bucket lengths ever start to get really high, itll be better to actually hash
    pub fn count_sort(&mut self, residencies: &[InstanceResidency]) {
        // build entity_count list, where entity_count[i] = number of entities
        // and i = render group index + instance bind key
        self.instance_to_bucket.clear();
        for res in residencies.iter() {
            if res.is_pending() {
                self.instance_to_bucket.push(u32::MAX);
                continue;
            }
            let bind_key = res.bind_key as u64;
            let bucket_key = ((res.group_id << 32) | bind_key) as u64;
            let idx = match self.bucket_keys.iter().position(|&k| k == bucket_key) {
                Some(i) => i,
                None => {
                    self.bucket_keys.push(bucket_key);
                    self.draw_buckets.push(DrawBucket {
                        group_idx: res.group_id as usize,
                        start: 0,
                        count: 0,
                    });
                    self.cursors.push(0);
                    self.draw_buckets.len() - 1
                }
            };
            //let idx = self.bucket_map.entry(bucket_key).or_insert_with(|| {
            //    self.draw_buckets.push(DrawBucket {
            //        group_idx: res.group_id as usize,
            //        start: 0,
            //        count: 0,
            //    });
            //    self.cursors.push(0);
            //    self.draw_buckets.len() - 1
            //});
            self.draw_buckets[idx].count += 1;
            self.instance_to_bucket.push(idx as u32);
        }
        // build instance_ranges, where instance_ranges[i] = the GPU shader instance idx range
        // and i = group + bind key index
        // cusors keeps track of the first instance of the entity associated with render_groups[i]
        let mut sum = 0;
        for (i, bucket) in self.draw_buckets.iter_mut().enumerate() {
            bucket.start = sum;
            self.cursors[i] = sum;
            sum += bucket.count;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pnu.is_empty() && self.pnujw.is_empty()
    }

    pub fn reset(&mut self, record_len: usize) {
        self.pnu.clear();
        self.pnujw.clear();
        self.cursors.clear();
        self.draw_buckets.clear();
        self.bucket_keys.clear();
        //self.bucket_map.clear();
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

    #[allow(unused)]
    pub fn from_u32(val: u32) -> Self {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RenderUpdateDelta {
    AssetUnloaded {
        key: u64,
    },
    AssetGPULoaded {
        key: u64,
        alloc_handle: GPUAllocationHandle,
    },
    InstanceSpawn {
        instance_key: u64,
        gpu_instance_handle: GPUInstanceHandle,
    },
    PrototypeCreated {
        entity_key: u64,
        prototype_handle: PrototypeHandle,
    },
    //EntitySpawned {
    //    instance_key: u64,
    //    gpu_instance_handle: GPUInstanceHandle,
    //    record_offset: u32,
    //    binding_key: InstanceBindKey,
    //},
    InstanceDespawn(GPUInstanceHandle),
}

#[derive(Debug, Clone, Copy)]
pub struct GPUInstanceHandle {
    pub(crate) instance_id: u32,
    pub(crate) prototype: PrototypeHandle,
    pub(crate) bind_id: u16,
}

#[cfg(test)]
impl GPUInstanceHandle {
    pub fn prototype_id(&self) -> u16 {
        self.prototype.0
    }
    pub fn instance_id(&self) -> u32 {
        self.instance_id
    }
}

impl Hash for GPUInstanceHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.instance_id.hash(state);
        self.prototype.hash(state);
    }
}

impl PartialEq for GPUInstanceHandle {
    fn eq(&self, other: &Self) -> bool {
        self.instance_id == other.instance_id && self.prototype == other.prototype
    }
}

impl Eq for GPUInstanceHandle {}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
struct AllocationMask(u32);

bitflags! {
    impl AllocationMask: u32 {
        const PNU_VERTEX = 0b00000001;
        const PNUJW_VERTEX = 0b00000010;
        const INDEX = 0b00000100;
        const TEX = 0b00001000;
        const MATERIAL = 0b00010000;
    }
}

#[derive(Clone, Debug)]
pub struct GPUAllocationHandle {
    global_allocation_id: u32,
    alloc_mask: AllocationMask,
}

impl PartialEq for GPUAllocationHandle {
    fn eq(&self, other: &Self) -> bool {
        self.global_allocation_id == other.global_allocation_id
    }
}

impl Eq for GPUAllocationHandle {}
impl Hash for GPUAllocationHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.global_allocation_id.hash(state);
    }
}

impl RenderKey for GPUAllocationHandle {
    fn as_key(&self) -> u64 {
        ((self.global_allocation_id as u64) << 32) | self.alloc_mask.bits() as u64
    }
    fn from_key(key: u64) -> Self {
        Self {
            global_allocation_id: (key >> 32) as u32,
            alloc_mask: AllocationMask::from_bits((key & 0xFFFFFFFF) as u32)
                .expect("should be a valid mask"),
        }
    }
}

#[cfg(test)]
impl GPUAllocationHandle {
    pub(crate) fn mock(global_allocation_id: u32) -> Self {
        Self {
            global_allocation_id,
            alloc_mask: AllocationMask::empty(),
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
    Dim2048,
}

impl TexDim {
    pub(in super::renderer) fn as_u32(self) -> u32 {
        match self {
            TexDim::Dim1 => 1,
            TexDim::Dim64 => 64,
            TexDim::Dim128 => 128,
            TexDim::Dim256 => 256,
            TexDim::Dim1024 => 1024,
            TexDim::Dim2048 => 2048,
        }
    }
    pub(crate) fn from_u32(val: u32) -> Self {
        match val {
            1 => Self::Dim1,
            64 => Self::Dim64,
            128 => Self::Dim128,
            256 => Self::Dim256,
            1024 => Self::Dim1024,
            2048 => Self::Dim2048,
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
    ReleasePrototype,
    AddAsset,
    SpawnInstance,
    LocalTransformUpload,
    JointTransformUpload,
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
    EmitInstanceSpawn,
    EmitPrototypeSpawn,
    //EmitEntitySpawn,
    DespawnInstance,
    DespawnAsset,
    Pop,
    Swap,
    PushKey,
    PushPrototype,
    PushInstance,
    PushAlloc,
}

#[derive(Debug)]
pub(crate) enum RenderConstant<'frame> {
    DataRef(&'frame [u8]),
    Key(u64),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TexLayer {
    bucket: u16,
    layer: u16,
}

#[derive(Debug, Clone)]
enum StackValue {
    Key(u64),
    Alloc(GPUAllocationHandle),
    Instance(GPUInstanceHandle),
    Offset(u32),
    TextureSlot(TexLayer),
    Prototype(PrototypeHandle),
}

impl StackValue {
    fn as_prototype(self) -> PrototypeHandle {
        match self {
            StackValue::Prototype(p) => p,
            _ => panic!("expected prototype handle, got {:?}", self),
        }
    }
    fn as_alloc(self) -> GPUAllocationHandle {
        match self {
            StackValue::Alloc(a) => a,
            StackValue::Key(a) => GPUAllocationHandle::from_key(a),
            _ => panic!("expected an alloc key, got {self:?}"),
        }
    }
    fn as_texture_slot(self) -> TexLayer {
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
    GpuUploadFailure(Box<dyn Error>),
}

impl From<VertexArenaError> for RenderUpdateError {
    fn from(value: VertexArenaError) -> Self {
        match value {
            _ => Self::GpuUploadFailure(Box::new(value)),
        }
    }
}

impl From<AllocationTableError> for RenderUpdateError {
    fn from(value: AllocationTableError) -> Self {
        return Self::GpuUploadFailure(Box::new(value));
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
