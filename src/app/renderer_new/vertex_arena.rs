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

struct GPUChunk<T: bytemuck::Pod> {
    remaining_space: u32,
    buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    allocator: FreeListAllocator,
    _t: PhantomData<T>,
}

#[derive(Hash, PartialEq, Eq)]
pub(super) struct AllocationHandle {
    global_alloc_id: u32,
    pipeline_alloc_id: u32,
}

impl GPUChunk<LocalTransform> {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            bind_group: None,
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
            bind_group: None,
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

pub(super) struct GPUArenaNew<T: bytemuck::Pod> {
    max_chunks: usize,
    chunks: Vec<GPUChunk<T>>,
    alloc_table: HashMap<u32, AllocMetaData>,
}

pub struct UploadMeshJob<'frame, V: ModelVertex> {
    pub verts: &'frame [V],
    pub(super) primitive_ranges: Vec<Range<u32>>,
    pub(super) per_model_primitive_count: Vec<u32>,
    pub(super) global_alloc_id: u32,
    pub(super) mesh_ids: Vec<u32>,
}

pub trait MeshUploadable<V: ModelVertex> {
    fn as_mesh_job(&self, global_alloc_id: u32) -> UploadMeshJob<V>;
}

impl GPUArenaNew<LocalTransform> {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            max_chunks: 1,
            chunks: vec![GPUChunk::<LocalTransform>::new(device)],
            alloc_table: HashMap::new(),
        }
    }
}

impl<V: ModelVertex> GPUArenaNew<V> {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            max_chunks: 16,
            chunks: vec![GPUChunk::<V>::new(device)],
            alloc_table: HashMap::new(),
        }
    }

    pub(super) fn resolve(&self, handle: &AllocationHandle) -> (Range<u32>, &wgpu::Buffer) {
        let meta = self.alloc_table.get(&handle.pipeline_alloc_id).unwrap();
        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        (range, &self.chunks[meta.chunk_id].buffer)
    }

    pub(super) fn upload_mesh(
        &mut self,
        mesh_data: &[V],
        global_alloc_id: u32,
        queue: &wgpu::Queue,
    ) -> Result<AllocationHandle, VertexArenaError> {
        'outer: for (chunk_idx, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(mesh_data, queue) {
                Ok((node_idx, _)) => {
                    let pipeline_alloc_id = self.alloc_table.len() as u32; // TODO: algorithm for assigning keys
                    self.alloc_table
                        .insert(pipeline_alloc_id, AllocMetaData::new(chunk_idx, node_idx));
                    return Ok(AllocationHandle {
                        global_alloc_id,
                        pipeline_alloc_id,
                    });
                }

                Err(e) => match e {
                    VertexArenaError::DataTooLarge(_) => {
                        return Err(e);
                    }
                    _ => continue 'outer,
                },
            }
        }
        Err(VertexArenaError::DataTooLarge(
            (mesh_data.len() * size_of::<V>()) as u32,
        ))
    }
}
