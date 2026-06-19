use std::num::NonZero;

use crate::{
    app::renderer::{
        InstanceUploadJob,
        bind_groups::{BindGroupProvider, SharedInstanceData},
        gpu_allocator::{GPUInstanceAllocator, VertexArenaError, instance_arena::InstanceArena},
        renderer::GPUInstanceHandle,
    },
    util::types::{LocalTransform, Mat4F32},
    world::instance_manager::InstanceHandle,
};
pub struct LocalTransformBindGroup {
    bind_groups: Vec<wgpu::BindGroup>,
    lt_arena: InstanceArena<LocalTransform>,
}

impl LocalTransformBindGroup {
    pub fn write_lt_anim_data(
        &mut self,
        handle: &GPUInstanceHandle,
        lt_data: &[u8],
        queue: &wgpu::Queue,
    ) {
        let buf = self.lt_arena.get_first_buffer();
        let offset = self.lt_arena.resolve(handle) as u64;
        queue.write_buffer(buf, offset, lt_data);
    }

    pub fn get_first_bg(&self) -> &wgpu::BindGroup {
        &self.bind_groups[0]
    }

    pub fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        let offset = self.lt_arena.upload(job, queue, device);
        if self.bind_groups.is_empty() {
            self.add_bind_group(device);
        }
        offset
    }

    pub fn register_shared_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        self.lt_arena
            .register_shared_binding(donor_handle, new_handle)
    }

    pub fn register_copy_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        self.lt_arena
            .register_copy_binding(donor_handle, new_handle, queue, device)
    }
}

impl BindGroupProvider for LocalTransformBindGroup {
    fn add_bind_group(&mut self, device: &wgpu::Device) {
        let bgl = Self::get_bind_group_layout(device);
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lt bind group"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: self.lt_arena.get_first_buffer(),
                    offset: 0,
                    size: None,
                }),
            }],
        });
        self.bind_groups.push(bg);
    }
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("LT bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: NonZero::<u64>::new(size_of::<Mat4F32>() as u64),
                },
                count: None,
            }],
        })
    }

    fn new() -> Self {
        let lts = InstanceArena::<LocalTransform>::new();
        Self {
            bind_groups: vec![],
            lt_arena: lts,
        }
    }

    fn get_bind_group(&self, handle: &InstanceHandle) -> &wgpu::BindGroup {
        // TODO: resolve based on alloc handle
        &self.bind_groups[0]
    }
}
