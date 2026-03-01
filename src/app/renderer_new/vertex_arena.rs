use std::fmt::{Display, Pointer};

use crate::{
    app::renderer_new::{
        CHUNK_SIZE,
        free_list::{FreeListAllocError, FreeListAllocator},
    },
    util::types::{ModelVertex, PNUJWVertex, PNUVertex},
};

pub(super) enum VertexArenaError {
    ChunkTooLarge(u64),
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
            Self::ChunkTooLarge(size) => f.write_str(
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
    remaining_space: u64,
    buffer: wgpu::Buffer,
    allocator: FreeListAllocator<V>,
}

impl<V: ModelVertex> VertexChunk<V> {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            remaining_space: CHUNK_SIZE,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: CHUNK_SIZE,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
            allocator: FreeListAllocator::new(),
        }
    }

    fn gpu_alloc(&mut self, data: &[V], queue: &wgpu::Queue) -> Result<(), VertexArenaError> {
        let size = (data.len() * size_of::<V>()) as u64;
        let mut offset: u64 = 0;
        if self.remaining_space >= size {
            offset = self.allocator.alloc_first(size)?;
        }
        queue.write_buffer(&self.buffer, offset, bytemuck::cast_slice(data));
        Ok(())
    }
}

pub(super) struct VertexArenaNew<V: ModelVertex> {
    chunks: Vec<VertexChunk<V>>,
}
pub struct UploadMeshJob<'frame, V: ModelVertex> {
    verts: &'frame [V],
}

pub trait MeshUploadable<V: ModelVertex> {
    fn to_raw_vertices(&self) -> UploadMeshJob<V> {
        todo!()
    }
}

impl<V: ModelVertex> VertexArenaNew<V> {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            chunks: vec![VertexChunk::new(device)],
        }
    }

    pub(super) fn upload_mesh(
        &mut self,
        mesh_data: UploadMeshJob<V>,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        'outer: for chunk in self.chunks.iter_mut() {
            match chunk.gpu_alloc(mesh_data.verts, queue) {
                Ok(_) => break,
                Err(e) => match e {
                    VertexArenaError::ChunkTooLarge(_) => {
                        return Err(e);
                    }
                    _ => continue 'outer,
                },
            }
        }
        todo!()
    }
}
