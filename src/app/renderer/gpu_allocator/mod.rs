use std::fmt::Debug;
use std::marker::PhantomData;
use std::{fmt::Display, ops::Range};

use bytemuck::Pod;
use std::error::Error;

use crate::app::renderer::bind_groups::BindGroupUploadResult;
use crate::app::renderer::gpu_allocator::free_list::FreeListAllocator;
use crate::app::renderer::renderer::GPUInstanceHandle;
use crate::app::renderer::{InstanceUploadJob, StorageData};
use crate::util::types::{
    GlobalTransform, InstanceOffset, InstanceRecordData, InverseBindMatrix, JointTransform,
    LocalTransform,
};
use crate::{
    app::renderer::GPUAllocationHandle, util::types::ModelVertex,
    world::instance_manager::InstanceHandle,
};

mod allocation_table;
mod free_list;
pub(super) mod instance_arena;
pub(super) mod vertex_arena;

static MIMIMUM_INDEX_ALLOCATION_SIZE: usize = 1024;
static MIMIMUM_VERTEX_ALLOCATION_SIZE: usize = 2048;

static CHUNK_SIZE: u32 = 1_048_576 * 8; //4 mb

#[derive(Debug)]
struct AllocMetaData {
    chunk_id: usize,
    node_id: usize,
    ref_count: usize,
}
pub(super) struct GPUChunk<T: bytemuck::Pod + Debug> {
    remaining_space: u32,
    buffer: wgpu::Buffer,
    allocator: FreeListAllocator,
    _t: PhantomData<T>,
}

impl<T: bytemuck::Pod + Debug> GPUChunk<T> {
    pub(super) fn gpu_alloc(
        &mut self,
        data: &[u8],
        queue: &wgpu::Queue,
        label: &str,
    ) -> Result<(usize, Range<u32>), VertexArenaError> {
        let size = data.len() as u32;

        let node_idx: usize = if self.remaining_space >= size {
            self.allocator.alloc_first(size)?
        } else {
            return Err(VertexArenaError::DataTooLarge(size, label.to_string()));
        };
        // for datum in data.iter().take(10) {
        //     println!("{:?}", datum);
        // }
        let offset = self.allocator.offset_of(node_idx) as u32;
        queue.write_buffer(&self.buffer, offset.into(), data);
        Ok((node_idx, offset..offset + (data.len() as u32)))
    }
}

pub(super) trait GPUAllocator<T: Pod> {
    type UploadJob<'a>;
    type AllocationError: Error;

    fn upload<'a>(
        &mut self,
        job: Self::UploadJob<'a>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), Self::AllocationError>;

    fn resolve(&self, handle: &GPUAllocationHandle) -> (Range<u32>, &wgpu::Buffer);

    fn new() -> Self;
}

pub(super) trait GPUInstanceAllocator<T: Pod> {
    type AllocationError: Error;

    fn purge_prototype_data(&mut self, slot_id: usize);
    fn upload<'a>(
        &mut self,
        job: InstanceUploadJob<'a, T>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<BindGroupUploadResult, Self::AllocationError>;

    fn resolve(&self, handle: &GPUInstanceHandle) -> u32;
    fn remove(&mut self, handle: &GPUInstanceHandle) -> Result<(), Self::AllocationError>;

    fn new() -> Self;

    #[allow(unused)]
    fn resolve_buffer(&self, instance_handle: &InstanceHandle) -> &wgpu::Buffer;
}
#[derive(Debug)]
pub enum FreeListAllocError {
    NoRoomLeft(u32, u32),
    NodeNotFount(usize),
}

impl Error for FreeListAllocError {}
impl Display for FreeListAllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRoomLeft(size, used) => f.write_str(
                format!(
                    "Not enough room to fit data of size {}. Largest Node Available: {}",
                    size, used,
                )
                .as_str(),
            ),
            Self::NodeNotFount(id) => write!(f, "node {} not found", id),
        }
    }
}

#[derive(Debug)]
pub enum VertexArenaError {
    DataTooLarge(u32, String),
    FreeListError(FreeListAllocError),
    HandleNotFound {
        shared: GPUInstanceHandle,
        donor: GPUInstanceHandle,
    },
    AllocationSlotNotFound,
    MetadataNotFound,
    MaxAllocationReached,
}

impl From<FreeListAllocError> for VertexArenaError {
    fn from(value: FreeListAllocError) -> Self {
        Self::FreeListError(value)
    }
}

impl Display for VertexArenaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataTooLarge(size, label) => f.write_str(
                format!(
                    "cannot allocate into {} mesh of size {}, which exceeds chunk size: {}",
                    label, size, CHUNK_SIZE
                )
                .as_str(),
            ),
            Self::FreeListError(err) => Display::fmt(&err, f),
            Self::MaxAllocationReached => f.write_str(
                "All Chunks are allocated, and there is no room in any of them for this upload",
            ),
            Self::HandleNotFound { shared, donor } => {
                write!(
                    f,
                    "tried to use handle: {:?} as a donor for {:?}, but the former was not found",
                    donor, shared
                )
            }
            Self::AllocationSlotNotFound => f.write_str("alloc slot not found"),
            Self::MetadataNotFound => f.write_str("No metadaat found at the slot"),
        }
    }
}

impl Error for VertexArenaError {}

pub(super) struct UploadMeshJob<'frame, V: ModelVertex> {
    pub verts: &'frame [u8],
    pub(super) global_alloc_id: u32,
    _t: PhantomData<V>,
}

impl<'frame, V: ModelVertex> UploadMeshJob<'frame, V> {
    pub(super) fn new(verts: &'frame [u8], alloc_id: u32) -> Self {
        Self {
            verts,
            global_alloc_id: alloc_id,
            _t: PhantomData,
        }
    }
}

pub(super) struct UploadIndexJob<'frame> {
    pub indices: &'frame [u8],
    pub(super) global_alloc_id: u32,
}

impl StorageData for LocalTransform {
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("local transform storage buffer"),
            size: CHUNK_SIZE as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        GPUChunk {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            buffer: buf,
            allocator: FreeListAllocator::new(size_of::<LocalTransform>()),
            _t: PhantomData,
        }
    }
}
impl StorageData for JointTransform {
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("joint transform storage buffer"),
            size: CHUNK_SIZE as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        GPUChunk {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            buffer: buf,
            allocator: FreeListAllocator::new(size_of::<LocalTransform>()),
            _t: PhantomData,
        }
    }
}
impl StorageData for InverseBindMatrix {
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("imb transform storage buffer"),
            size: CHUNK_SIZE as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        GPUChunk {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            buffer: buf,
            allocator: FreeListAllocator::new(size_of::<LocalTransform>()),
            _t: PhantomData,
        }
    }
}

impl StorageData for GlobalTransform {
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GT buffer"),
            size: (CHUNK_SIZE / 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        GPUChunk {
            remaining_space: CHUNK_SIZE / 4,
            buffer: buf,
            allocator: FreeListAllocator::new(size_of::<GlobalTransform>()),
            _t: PhantomData,
        }
    }
}

impl StorageData for InstanceRecordData {
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance record storage buffer"),
            size: CHUNK_SIZE as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        GPUChunk {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            buffer: buf,
            allocator: FreeListAllocator::new(size_of::<InstanceRecordData>()),
            _t: PhantomData,
        }
    }
}

impl StorageData for InstanceOffset {
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance offset storage buffer"),
            size: CHUNK_SIZE as u64 / 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        GPUChunk {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            buffer: buf,
            allocator: FreeListAllocator::new(size_of::<InstanceOffset>()),
            _t: PhantomData,
        }
    }
}
