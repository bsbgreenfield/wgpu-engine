use std::{collections::HashMap, fmt::Display, ops::Range};

use crate::{
    app::renderer_new::{
        CHUNK_SIZE,
        free_list::{FreeListAllocError, FreeListAllocator},
    },
    util::types::ModelVertex,
};

pub(super) enum VertexArenaError {
    DataTooLarge(u64),
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

struct VertexChunk<V: ModelVertex> {
    remaining_space: u32,
    buffer: wgpu::Buffer,
    allocator: FreeListAllocator<V>,
}

pub(super) struct AllocationHandle {
    cache_id: usize,
    alloc_id: usize,
}

impl<V: ModelVertex> VertexChunk<V> {
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
        }
    }

    fn gpu_alloc(&mut self, data: &[V], queue: &wgpu::Queue) -> Result<usize, VertexArenaError> {
        let size = (data.len() * size_of::<V>()) as u32;
        let node_idx: usize = if self.remaining_space >= size {
            self.allocator.alloc_first(size)?
        } else {
            return Err(VertexArenaError::DataTooLarge(size));
        };
        queue.write_buffer(
            &self.buffer,
            self.allocator.offset_of(node_idx),
            bytemuck::cast_slice(data),
        );
        Ok(node_idx)
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

pub(super) struct VertexArenaNew<V: ModelVertex> {
    chunks: Vec<VertexChunk<V>>,
    alloc_table: HashMap<usize, AllocMetaData>,
}

pub struct UploadMeshJob<'frame, V: ModelVertex> {
    pub verts: &'frame [V],
}

pub trait MeshUploadable<V: ModelVertex> {
    fn to_raw_vertices(&self) -> UploadMeshJob<V>;
}

impl<V: ModelVertex> VertexArenaNew<V> {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            chunks: vec![VertexChunk::new(device)],
            alloc_table: HashMap::new(),
        }
    }

    pub(super) fn resolve(&self, handle: &AllocationHandle) -> Range<u32> {
        let meta = self.alloc_table.get(&handle.alloc_id).unwrap();
        self.chunks[meta.chunk_id].allocator.resolve(meta.node_id)
    }

    pub(super) fn upload_mesh(
        &mut self,
        mesh_data: UploadMeshJob<V>,
        queue: &wgpu::Queue,
    ) -> Result<AllocationHandle, VertexArenaError> {
        let mut chunk_idx;
        'outer: for (idx, chunk) in self.chunks.iter_mut().enumerate() {
            chunk_idx = idx;
            match chunk.gpu_alloc(mesh_data.verts, queue) {
                Ok(node_idx) => {
                    let k = self.alloc_table.len(); // TODO: algorithm for assigning keys
                    self.alloc_table
                        .insert(k, AllocMetaData::new(chunk_idx, node_idx));
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
