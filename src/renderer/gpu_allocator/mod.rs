use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::range::Range;

use std::error::Error;

use crate::renderer::GPUAllocationHandle;
use crate::renderer::GPUInstanceHandle;
use crate::renderer::GPUUploadable;
use crate::renderer::StorageData;
use crate::renderer::gpu_allocator::free_list::FreeListAllocator;
use crate::util::types::ModelVertex;
use crate::util::types::{
    GlobalTransform, InstanceOffset, InstanceRecordData, InverseBindMatrix, JointTransform,
    LocalTransform,
};

mod allocation_table;
mod free_list;
pub(super) mod gpu_arena;
pub(super) mod instance_arena;

static MIMIMUM_INDEX_ALLOCATION_SIZE: usize = 1024;
static MIMIMUM_VERTEX_ALLOCATION_SIZE: usize = 2048;

static CHUNK_SIZE: u32 = 1_048_576 * 8; //4 mb

#[derive(Debug, Clone)]
struct AllocMetaData {
    chunk_id: usize,
    node_id: usize,
    ref_count: usize,
}
// pub(crate): return type of `GPUUploadable::get_chunk`, which is pub(crate).
pub(crate) struct GPUChunk<T: bytemuck::Pod + Debug> {
    remaining_space: u32,
    buffer: wgpu::Buffer,
    allocator: FreeListAllocator,
    _t: PhantomData<T>,
}

impl<T: GPUUploadable + bytemuck::Pod + Debug> GPUChunk<T> {
    fn gpu_alloc(
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
        Ok((node_idx, Range::from(offset..offset + (data.len() as u32))))
    }

    pub fn new(device: &wgpu::Device, size: u32, label: &str, usages: wgpu::BufferUsages) -> Self {
        Self {
            remaining_space: size,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: usages,
                mapped_at_creation: false,
            }),
            allocator: FreeListAllocator::new(T::CHUNK_SIZE, MIMIMUM_INDEX_ALLOCATION_SIZE),
            _t: PhantomData,
        }
    }
}

// pub(crate): return type of `GPUUploadable::upload`, which is pub(crate).
pub(crate) enum GPUUploadResult {
    BindGroupUploadResult {
        buffer_element_offset: u32,
        alloc_meta_idx: usize,
    },
    VertexDataUploadSuccess,
}
pub(super) trait GPUAllocator<T: GPUUploadable> {
    type AllocationError: Error;

    fn upload<'a>(
        &mut self,
        job: T::UploadJob<'a>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, Self::AllocationError>;

    fn resolve(&self, handle: &T::GPUHandle) -> (Range<u32>, &wgpu::Buffer);

    fn remove(&mut self, handle: &T::GPUHandle) -> Result<(), Self::AllocationError>;

    fn new() -> Self;

    fn dealloc(&mut self, handle: &T::GPUHandle) -> Result<(), Self::AllocationError>;
}

// pub(crate): wrapped by `VertexArenaError::FreeListError`, which is pub(crate).
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

// pub(crate): `GPUUploadable::UploadJob` for PNU/PNUJW vertex uploads.
pub(crate) struct UploadMeshJob<'frame, V: ModelVertex> {
    verts: &'frame [u8],
    alloc_handle: GPUAllocationHandle,
    _t: PhantomData<V>,
}

impl<'frame, V: ModelVertex> UploadMeshJob<'frame, V> {
    pub(super) fn new(verts: &'frame [u8], alloc_handle: GPUAllocationHandle) -> Self {
        Self {
            verts,
            alloc_handle,
            _t: PhantomData,
        }
    }
}

// pub(crate): `GPUUploadable::UploadJob` for index uploads.
pub(crate) struct UploadIndexJob<'frame> {
    pub(super) indices: &'frame [u8],
    pub(super) alloc_handle: GPUAllocationHandle,
}

impl StorageData for LocalTransform {
    const LABEL: &'static str = "Local Transform data";

    const CHUNK_SIZE: u32 = CHUNK_SIZE;
}
impl StorageData for JointTransform {
    const LABEL: &'static str = "Joint Transform Data";

    const CHUNK_SIZE: u32 = CHUNK_SIZE;
}
impl StorageData for InverseBindMatrix {
    const LABEL: &'static str = "IBM Data";

    const CHUNK_SIZE: u32 = CHUNK_SIZE;
}

impl StorageData for GlobalTransform {
    const LABEL: &'static str = " GlobalTransform data";

    const CHUNK_SIZE: u32 = CHUNK_SIZE;
}

impl StorageData for InstanceRecordData {
    const LABEL: &'static str = " Instance Record Data";

    const CHUNK_SIZE: u32 = CHUNK_SIZE;
}

impl StorageData for InstanceOffset {
    const LABEL: &'static str = "Instance Offset data";

    const CHUNK_SIZE: u32 = CHUNK_SIZE;
}

// pub(crate): bound on `GPUUploadable::UploadJob`, which is pub(crate).
pub(crate) trait GPUUploadJob {
    type GPUHandle: Eq + Debug + Clone + Hash;
    fn get_data(&self) -> &[u8];
    fn get_handle(&self) -> Self::GPUHandle;
}
