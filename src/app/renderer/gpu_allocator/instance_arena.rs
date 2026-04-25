use std::{fmt::Debug, marker::PhantomData};

use crate::{
    app::renderer::gpu_allocator::{
        CHUNK_SIZE, GPUInstanceAllocator, LocalTransformUploadJob, VertexArenaError,
        free_list::FreeListAllocator, vertex_arena::GPUChunk,
    },
    util::types::LocalTransform,
};

struct InstanceChunk<T: bytemuck::Pod + Debug> {
    remaining_space: u32,
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    allocator: FreeListAllocator,
    _t: PhantomData<T>,
}

pub struct InstanceArena<T: bytemuck::Pod + Debug> {
    max_chunks: usize,
    chunks: Vec<GPUChunk<T>>,
    allocator: FreeListAllocator,
}

impl InstanceChunk<LocalTransform> {
    fn new(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout) -> Self {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("local transform storage buffer"),
            size: CHUNK_SIZE as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let new_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lt bind group"),
            layout: bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        });
        Self {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            bind_group: new_bg,
            buffer: buf,
            allocator: FreeListAllocator::new(),
            _t: PhantomData,
        }
    }
}

impl GPUInstanceAllocator<LocalTransform> for InstanceArena<LocalTransform> {
    type UploadJob<'a> = LocalTransformUploadJob<'a>;

    type AllocationError = VertexArenaError;

    fn upload<'a>(
        self,
        job: Self::UploadJob<'a>,
        queue: &wgpu::Queue,
    ) -> Result<(), Self::AllocationError> {
        todo!()
    }

    fn resolve(
        &self,
        handle: &crate::world::instance_manager::InstanceHandle,
    ) -> (u32, &wgpu::BindGroup) {
        todo!()
    }

    fn new(device: &wgpu::Device) -> Self {
        todo!()
    }
}
