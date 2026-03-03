use std::{collections::HashMap, fmt::Display, marker::PhantomData, ops::Range};

use crate::{
    app::renderer_new::{
        CHUNK_SIZE,
        free_list::{FreeListAllocError, FreeListAllocator},
    },
    util::types::{LocalTransform, ModelVertex},
};

pub(super) enum VertexArenaError {
    DataTooLarge(u32),
    FreeListError(FreeListAllocError),
}

impl From<FreeListAllocError> for VertexArenaError {
    fn from(value: FreeListAllocError) -> Self {
        Self::FreeListError(value)
    }
}

impl Display for VertexArenaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::DataTooLarge(size) => f.write_str(
                format!(
                    "cannot allocate mesh of size {}, which exceeds chunk size: {}",
                    size, CHUNK_SIZE
                )
                .as_str(),
            ),
            Self::FreeListError(err) => err.fmt(f),
        }
    }
}

struct GPUChunk<T> {
    remaining_space: u32,
    buffer: wgpu::Buffer,
    allocator: FreeListAllocator,
    _t: PhantomData<T>,
}

pub(super) struct AllocationHandle {
    pub(super) cache_id: usize,
    alloc_id: usize,
}

impl GPUChunk<LocalTransform> {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: CHUNK_SIZE as u64,
                usage: wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            allocator: FreeListAllocator::new(),
            _t: PhantomData,
        }
    }
}

impl<T: bytemuck::Pod> GPUChunk<T> {
    fn gpu_alloc(
        &mut self,
        data: &[T],
        queue: &wgpu::Queue,
    ) -> Result<(usize, Range<u32>), VertexArenaError> {
        let size = (data.len() * size_of::<T>()) as u32;
        let node_idx: usize = if self.remaining_space >= size {
            self.allocator.alloc_first(size)?
        } else {
            return Err(VertexArenaError::DataTooLarge(size));
        };
        let offset = self.allocator.offset_of(node_idx) as u32;
        queue.write_buffer(&self.buffer, offset.into(), bytemuck::cast_slice(data));
        Ok((node_idx, offset..offset + size))
    }
}

impl<T: ModelVertex> GPUChunk<T> {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            remaining_space: CHUNK_SIZE,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: CHUNK_SIZE as u64,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
            allocator: FreeListAllocator::new(),
            _t: PhantomData,
        }
    }
}

struct AllocMetaData {
    chunk_id: usize,
    node_id: usize,
}
impl AllocMetaData {
    fn new(chunk_id: usize, node_id: usize) -> Self {
        Self { chunk_id, node_id }
    }
}

struct DrawCacheChunk {
    pub(super) vertex_ranges: Vec<Range<u32>>,
    valid: bool,
}

pub(super) struct GPUArenaNew<T> {
    max_chunks: usize,
    pub(super) chunk_caches: Vec<DrawCacheChunk>,
    chunks: Vec<GPUChunk<T>>,
    alloc_table: HashMap<usize, AllocMetaData>,
}

pub struct UploadMeshJob<'frame, V: ModelVertex> {
    pub verts: &'frame [V],
}

pub trait MeshUploadable<V: ModelVertex> {
    fn to_raw_vertices(&self) -> UploadMeshJob<V>;
}

impl GPUArenaNew<LocalTransform> {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            max_chunks: 1,
            chunk_caches: vec![],
            chunks: vec![GPUChunk::<LocalTransform>::new(device)],
            alloc_table: HashMap::new(),
        }
    }
}

impl<V: ModelVertex> GPUArenaNew<V> {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            max_chunks: 16,
            chunk_caches: vec![],
            chunks: vec![GPUChunk::<V>::new(device)],
            alloc_table: HashMap::new(),
        }
    }
    pub(super) fn get_chunk_draws<'frame>(
        &'frame self,
    ) -> impl Iterator<Item = (&wgpu::Buffer, &[Range<u32>])> {
        self.chunk_caches
            .iter()
            .zip(self.chunks.iter())
            .map(|(a, b)| {
                return (&b.buffer, &a.vertex_ranges[..]);
            })
    }

    pub(super) fn resolve(&self, handle: &AllocationHandle) -> (Range<u32>, &wgpu::Buffer) {
        let meta = self.alloc_table.get(&handle.alloc_id).unwrap();
        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        (range, &self.chunks[meta.chunk_id].buffer)
    }

    pub(super) fn upload_mesh(
        &mut self,
        mesh_data: UploadMeshJob<V>,
        queue: &wgpu::Queue,
    ) -> Result<AllocationHandle, VertexArenaError> {
        'outer: for (chunk_idx, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(mesh_data.verts, queue) {
                Ok((node_idx, range)) => {
                    let k = self.alloc_table.len(); // TODO: algorithm for assigning keys
                    self.alloc_table
                        .insert(k, AllocMetaData::new(chunk_idx, node_idx));
                    self.chunk_caches[chunk_idx].vertex_ranges.push(range);
                    break 'outer;
                }

                Err(e) => match e {
                    VertexArenaError::DataTooLarge(_) => {
                        return Err(e);
                    }
                    _ => continue 'outer,
                },
            }
        }
        Ok(AllocationHandle {
            cache_id: 0, //TODO: implement cache
            alloc_id: 0,
        })
    }
}
