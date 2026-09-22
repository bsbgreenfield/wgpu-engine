use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::range::Range;

use std::error::Error;

use crate::renderer::GPUAllocationHandle;
use crate::renderer::GPUInstanceHandle;
use crate::renderer::PrototypeHandle;
use crate::renderer::TexDim;
use crate::renderer::gpu_allocator::allocation_tables::StorageData;
use crate::renderer::gpu_allocator::allocation_tables::TAllocationTable;
use crate::renderer::gpu_allocator::free_list::FreeListAllocator;
use crate::renderer::gpu_allocator::gpu_arena::GPUArena;
use crate::util::types::ModelVertex;

mod allocation_tables;
mod free_list;
pub(super) mod gpu_arena;
pub(super) mod texture_arena;

static CHUNK_SIZE: u32 = 1_048_576 * 8; //4 mb

pub(super) fn get_per_frame_buffer<S>(device: &wgpu::Device) -> wgpu::Buffer
where
    S: StorageData,
{
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("instance offset buffer"),
        size: 1024 * 4,
        usage: <S as StorageData>::BUFFER_USAGES,
        mapped_at_creation: false,
    })
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
            return Err(VertexArenaError::DataTooLarge(
                size,
                label.to_string(),
                T::CHUNK_SIZE,
            ));
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
            allocator: FreeListAllocator::new(T::CHUNK_SIZE, T::MIN_ALLOC_SIZE as usize),
            _t: PhantomData,
        }
    }
}

// pub(crate): return type of `GPUUploadable::upload`, which is pub(crate).
pub(crate) enum GPUUploadResult {
    BindGroupUploadResult {
        buffer_element_offset: u32,
        chunk_idx: u32,
    },
    InstanceRecordUpload,
    PrototypeUploaded,
    VertexDataUploadSuccess,
    MaterialUploadSucess,
    TextureUploadSuccess,
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

    fn new() -> Self;

    fn dealloc(&mut self, handle: &T::GPUHandle) -> Result<(), Self::AllocationError>;
}

// pub(crate): wrapped by `VertexArenaError::FreeListError`, which is pub(crate).
#[derive(Debug)]
pub(crate) enum FreeListAllocError {
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
pub(crate) enum VertexArenaError {
    DeallocError,
    DataTooLarge(u32, String, u32),
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
            Self::DataTooLarge(size, label, max_chunk_size) => f.write_str(
                format!(
                    "cannot allocate into {} mesh of size {}, which exceeds chunk size: {}",
                    label, size, max_chunk_size
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
            Self::DeallocError => f.write_str("dealloc failure"),
        }
    }
}

impl Error for VertexArenaError {}

pub(crate) struct UploadMaterialJob<'frame> {
    pub(super) data: &'frame [u8],
    pub(super) alloc_handle: GPUAllocationHandle,
}

pub(crate) struct UploadTextureJob<'frame> {
    pub(super) pixels: &'frame [u8],
    pub(super) dim: TexDim,
    pub(super) texture_handle: GPUAllocationHandle,
}

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

// pub(crate): bound on `GPUUploadable::UploadJob`, which is pub(crate).
pub(crate) trait GPUUploadJob {
    type GPUHandle: Eq + Debug + Clone + Hash;
    fn get_data(&self) -> &[u8];
    fn get_handle(&self) -> Self::GPUHandle;
}
pub(in crate::renderer) trait GPUUploadable: Debug + bytemuck::Pod {
    type GPUHandle: Debug + Clone + Hash + Eq;
    type AllocTable: TAllocationTable<Handle = Self::GPUHandle>;
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
