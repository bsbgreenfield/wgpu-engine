use crate::renderer::{
    InstanceUploadJob, bind_groups::BindGroupUploadResult, gpu_allocator::VertexArenaError,
    renderer::GPUInstanceHandle,
};
use std::num::NonZero;

use crate::{
    renderer::{bind_groups::BindGroupProvider, gpu_allocator::instance_arena::InstanceArena},
    util::types::{GlobalTransform, InstanceOffset, InstanceRecordData},
    world::instance_manager::InstanceHandle,
};

pub struct InstanceDataBindGroup {
    bind_groups: Vec<wgpu::BindGroup>,
    record_arena: InstanceArena<InstanceRecordData>,
    offsets: InstanceArena<InstanceOffset>,
    global_transforms: InstanceArena<GlobalTransform>,
}

impl BindGroupProvider for InstanceDataBindGroup {
    fn get_bind_group(&self, alloc_handle: &InstanceHandle) -> &wgpu::BindGroup {
        self.bind_groups.first().expect("bind group does not exist")
    }

    fn add_bind_group(&mut self, device: &wgpu::Device) {
        // TODO: formalize the system for lazy allocating all bg buffers
        self.allocate_buffers(device);

        let bgl = Self::get_bind_group_layout(device);
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("instance Data BG"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: self.record_arena.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: self.offsets.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: self.global_transforms.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });
        self.bind_groups.push(bg);
    }
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("instance data bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::<u64>::new(16),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::<u64>::new(4),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::<u64>::new(64),
                    },
                    count: None,
                },
            ],
        })
    }

    fn new() -> Self {
        let instance_records = InstanceArena::<InstanceRecordData>::new();
        let instance_offsets = InstanceArena::<InstanceOffset>::new();
        let global_transforms = InstanceArena::<GlobalTransform>::new();

        Self {
            bind_groups: vec![],
            record_arena: instance_records,
            offsets: instance_offsets,
            global_transforms,
        }
    }

    fn despawn(&mut self, handle: &GPUInstanceHandle) {
        // all of these are unique per instance,
        // so when despawn is called, the data needs to actually be removed
        // reather than just decrementing a ref count
        let _ = self.record_arena.remove(handle);
        let _ = self.global_transforms.remove(handle);
        let _ = self.offsets.remove(handle);
    }
}

impl InstanceDataBindGroup {
    pub fn get_first_bg(&self) -> &wgpu::BindGroup {
        &self.bind_groups[0]
    }
    fn allocate_buffers(&mut self, device: &wgpu::Device) {
        self.offsets.add_buffer(device);
        self.global_transforms.add_buffer(device);
    }
    pub fn write_gt_data(&self, data: &[u8], queue: &wgpu::Queue) {
        let buf = self.global_transforms.get_first_buffer();
        queue.write_buffer(buf, 0, data);
    }
    pub fn upload_instance_record<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, InstanceRecordData>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<BindGroupUploadResult, VertexArenaError> {
        let upload_result = self.record_arena.upload(job, queue, device);
        if self.bind_groups.is_empty() {
            self.add_bind_group(device);
        }
        upload_result
    }
    pub fn upload_instance_offsets<'frame>(
        &mut self,
        offset_data: &'frame [u32],
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(
            self.offsets.get_first_buffer(),
            0,
            bytemuck::cast_slice(offset_data),
        );
    }
}
