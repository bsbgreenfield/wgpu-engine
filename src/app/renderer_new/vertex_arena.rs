use crate::{
    app::renderer_new::{CHUNK_SIZE, free_list::FreeListAllocator},
    util::types::ModelVertex,
};

struct VertexChunk<V: ModelVertex> {
    buffer: wgpu::Buffer,
    allocator: FreeListAllocator<V>,
}

impl<V: ModelVertex> VertexChunk<V> {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: CHUNK_SIZE,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
            allocator: FreeListAllocator::new(),
        }
    }
}

pub(super) struct VertexArenaNew<V: ModelVertex> {
    chunks: Vec<VertexChunk<V>>,
}

impl<V: ModelVertex> VertexArenaNew<V> {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            chunks: vec![VertexChunk::new(device)],
        }
    }
}
