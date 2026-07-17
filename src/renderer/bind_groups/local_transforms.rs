use std::num::NonZero;

use crate::{
    common::instance::{GPUInstanceHandle, InstanceHandle},
    renderer::{
        InstanceUploadJob,
        bind_groups::{BindGroupProvider, BindGroupUploadResult},
        gpu_allocator::{VertexArenaError, instance_arena::InstanceArena},
    },
    util::types::{LocalTransform, Mat4F32},
};
pub struct LocalTransformBindGroup {
    bind_groups: Vec<wgpu::BindGroup>,
    lt_arena: InstanceArena<LocalTransform>,
}

impl LocalTransformBindGroup {
    pub fn get_first_buffer(&self) -> &wgpu::Buffer {
        self.lt_arena.get_first_buffer()
    }
    pub fn write_lt_anim_data(
        &mut self,
        handle: &GPUInstanceHandle,
        lt_data: &[u8],
        queue: &wgpu::Queue,
    ) {
        let buf = self.lt_arena.get_first_buffer();
        let offset = self.lt_arena.resolve_byte_offset(handle) as u64;
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
    ) -> Result<BindGroupUploadResult, VertexArenaError> {
        let upload_result = self.lt_arena.upload(job, queue, device);
        if self.bind_groups.is_empty() {
            self.add_bind_group(device);
        }
        upload_result
    }

    pub fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        self.lt_arena
            .register_shared_binding(slot_index, new_handle)
    }

    pub fn register_copy_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        self.lt_arena
            .register_copy_binding(slot_index, new_handle, queue, device)
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

    fn despawn(&mut self, handle: &GPUInstanceHandle) {
        let _ = self.lt_arena.dealloc(handle);
    }
}
