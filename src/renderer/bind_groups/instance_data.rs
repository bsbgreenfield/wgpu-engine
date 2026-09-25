use crate::{
    common::instance::InstanceHandle,
    renderer::{
        GPUInstanceHandle, InstanceUploadJob, RenderUpdateError,
        gpu_allocator::{
            GPUUploadResult, VertexArenaError, allocation_tables::AllocationTableError,
            gpu_arena::GPUArena,
        },
    },
};
use std::num::NonZero;

use crate::{
    renderer::{bind_groups::BindGroupProvider, gpu_allocator::GPUAllocator},
    util::types::{GlobalTransform, InstanceOffset, InstanceRecordData},
};

pub(in crate::renderer) struct InstanceDataBindGroup {
    bind_groups: Vec<wgpu::BindGroup>,
    record_arena: GPUArena<InstanceRecordData>,
    offsets: Option<wgpu::Buffer>,
    global_transforms: Option<wgpu::Buffer>,
}

impl BindGroupProvider for InstanceDataBindGroup {
    #[allow(unused)]
    fn get_bind_group(&self, alloc_handle: &InstanceHandle) -> &wgpu::BindGroup {
        self.bind_groups.first().expect("bind group does not exist")
    }

    fn update_bind_group(&mut self, device: &wgpu::Device, bg_idx: usize) {
        todo!()
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
                        buffer: self.offsets.as_ref().unwrap(),
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: self.global_transforms.as_ref().unwrap(),
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
        let instance_records = GPUArena::<InstanceRecordData>::new();

        Self {
            bind_groups: vec![],
            record_arena: instance_records,
            offsets: None,
            global_transforms: None,
        }
    }

    fn despawn(&mut self, handle: &GPUInstanceHandle) {
        self.record_arena
            .dealloc(handle)
            .expect("instance despawn failed");
    }
}

impl InstanceDataBindGroup {
    #[cfg(test)]
    pub(in crate::renderer) fn get_first_record_buffer(&self) -> &wgpu::Buffer {
        self.record_arena.get_first_buffer()
    }
    pub(in crate::renderer) fn get_first_bg(&self) -> &wgpu::BindGroup {
        &self.bind_groups[0]
    }
    fn allocate_buffers(&mut self, device: &wgpu::Device) {
        let offset_buf =
            crate::renderer::gpu_allocator::get_per_frame_buffer::<InstanceOffset>(device);
        let gt_buff =
            crate::renderer::gpu_allocator::get_per_frame_buffer::<GlobalTransform>(device);
        self.offsets = Some(offset_buf);
        self.global_transforms = Some(gt_buff)
    }
    pub(in crate::renderer) fn write_gt_data(&self, data: &[u8], queue: &wgpu::Queue) {
        let buf = self.global_transforms.as_ref().unwrap();
        queue.write_buffer(buf, 0, data);
    }
    pub(in crate::renderer) fn upload_instance_record<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, InstanceRecordData>,
        node_id: u32,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let upload_result = self.record_arena.upload_reserved(job, node_id, queue);
        if self.bind_groups.is_empty() {
            self.add_bind_group(device);
        }
        Ok(upload_result)
    }
    pub(in crate::renderer) fn upload_instance_offsets<'frame>(
        &mut self,
        offset_data: &'frame [u32],
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(
            self.offsets.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(offset_data),
        );
    }

    pub(in crate::renderer) fn reserve_record_slot(
        &mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(usize, usize), AllocationTableError> {
        self.record_arena.reserve(queue, device)
    }
}
