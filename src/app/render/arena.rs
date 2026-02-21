use std::{any::TypeId, marker::PhantomData, ops::Range};

use crate::{
    app::render::{GPUMeshHandle, UploadMeshJob},
    util::types::{IndexType, ModelVertex, PNUJWVertex},
};

struct ArenaAllocator {
    cursor: u64,
}

impl ArenaAllocator {
    fn alloc(&mut self, size: u64, align: u64) -> Option<Range<u64>> {
        let aligned = Self::align_up(self.cursor, align);
        let end = aligned + size;
        self.cursor = end;
        Some(aligned..end)
    }

    fn align_up(value: u64, align: u64) -> u64 {
        (value + align - 1) & !(align - 1)
    }
}

pub(super) struct GPUMeshArena<V: ModelVertex, I: IndexType> {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_capacity: u64,
    vertex_allocator: ArenaAllocator,
    index_allocator: ArenaAllocator,
    vertex_type: PhantomData<V>,
    index_type: PhantomData<I>,
}
impl<V: ModelVertex> GPUMeshArena<V, u16> {
    pub(super) fn new(device: &wgpu::Device, vertex_capacity: u64, index_capacity: u64) -> Self {
        let label: String = format!("vertex buffer arena for {:?}", TypeId::of::<V>());
        GPUMeshArena {
            vertex_capacity,
            index_capacity,
            vertex_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&label),
                size: vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            index_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("PNUJ vertex buffer arena"),
                size: vertex_capacity,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            vertex_allocator: ArenaAllocator { cursor: 0 },
            index_allocator: ArenaAllocator { cursor: 0 },
            vertex_type: PhantomData::<V>,
            index_type: PhantomData::<u16>,
        }
    }

    pub(super) fn upload_mesh(
        &mut self,
        upload_job: UploadMeshJob<V, u16>,
        queue: &wgpu::Queue,
    ) -> Option<GPUMeshHandle> {
        let vertex_range = self.vertex_allocator.alloc(
            upload_job.vertices.len() as u64,
            std::mem::align_of::<PNUJWVertex>() as u64,
        )?;
        let index_range = self.index_allocator.alloc(
            upload_job.indices.len() as u64,
            std::mem::align_of::<u16>() as u64,
        )?;
        queue.write_buffer(
            &self.vertex_buffer,
            vertex_range.start,
            bytemuck::cast_slice(upload_job.vertices),
        );

        queue.write_buffer(
            &self.index_buffer,
            index_range.start,
            bytemuck::cast_slice(upload_job.indices),
        );

        Some(GPUMeshHandle {
            vertex: vertex_range,
            index: index_range,
            count: upload_job.indices.len() as u64,
        })
    }
}
