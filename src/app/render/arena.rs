use std::{any::TypeId, marker::PhantomData, ops::Range};

use crate::{
    app::render::GPUMeshHandle,
    asset_manager::asset_manager::AssetHandle,
    util::types::{IndexType, ModelVertex, PNUJWVertex, PNUVertex},
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

pub(super) struct GPUMeshArena {
    pub(super) pnujw_vertex_buffer: wgpu::Buffer,
    pub(super) pnu_vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    pnujw_capacity: u64,
    pnu_capacity: u64,
    index_capacity: u64,
    pnu_allocator: ArenaAllocator,
    pnujw_allocator: ArenaAllocator,
    index_allocator: ArenaAllocator,
}

impl GPUMeshArena {
    pub(super) fn new(
        label: Option<&str>,
        device: &wgpu::Device,
        vertex_capacity: u64,
        index_capacity: u64,
    ) -> Self {
        GPUMeshArena {
            index_capacity,
            pnujw_capacity: vertex_capacity,
            pnu_capacity: vertex_capacity,
            pnujw_vertex_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label,
                size: vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            pnu_vertex_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label,
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
            pnu_allocator: ArenaAllocator { cursor: 0 },
            pnujw_allocator: ArenaAllocator { cursor: 0 },
            index_allocator: ArenaAllocator { cursor: 0 },
        }
    }

    pub(super) fn upload_mesh(
        &mut self,
        asset_handle: AssetHandle,
        pnujw_verts: Option<&[PNUJWVertex]>,
        pnu_verts: Option<&[PNUVertex]>,
        indices: Option<&[u16]>,
        queue: &wgpu::Queue,
    ) -> Option<GPUMeshHandle> {
        let mut handle: GPUMeshHandle = GPUMeshHandle {
            handle: asset_handle,
            count: 0,
            vertex_pnu: Range { start: 0, end: 0 },
            vertex_pnujw: Range { start: 0, end: 0 },
            index: Range { start: 0, end: 0 },
        };
        if let Some(pnujw_verts) = pnujw_verts {
            let pnujw_range = self.pnujw_allocator.alloc(
                pnujw_verts.len() as u64,
                std::mem::align_of::<PNUJWVertex>() as u64,
            )?;
            handle.vertex_pnujw = pnujw_range.clone();

            queue.write_buffer(
                &self.pnujw_vertex_buffer,
                pnujw_range.start,
                bytemuck::cast_slice(pnujw_verts),
            );
        }

        if let Some(pnu_verts) = pnu_verts {
            let pnu_range = self.pnujw_allocator.alloc(
                pnu_verts.len() as u64,
                std::mem::align_of::<PNUVertex>() as u64,
            )?;

            handle.vertex_pnu = pnu_range.clone();

            queue.write_buffer(
                &self.pnu_vertex_buffer,
                pnu_range.start,
                bytemuck::cast_slice(pnu_verts),
            );
        }
        if let Some(indices) = indices {
            let index_range = self
                .index_allocator
                .alloc(indices.len() as u64, std::mem::align_of::<u16>() as u64)?;

            handle.index = index_range.clone();

            queue.write_buffer(
                &self.index_buffer,
                index_range.start,
                bytemuck::cast_slice(indices),
            );
        }

        Some(handle)
    }
}
