use std::num::NonZero;

use crate::{
    common::instance::InstanceHandle,
    renderer::{
        GPUInstanceHandle, InstanceUploadJob,
        bind_groups::{BindGroupProvider, SharedInstanceBindGroup},
        gpu_allocator::{
            GPUAllocator, GPUUploadResult, VertexArenaError,
            allocation_tables::AllocationTableError,
            gpu_arena::{GPUArena, InstanceAllocationResult},
        },
    },
    util::types::{LocalTransform, Mat4F32},
};
pub(in crate::renderer) struct LocalTransformBindGroup {
    bind_groups: Vec<wgpu::BindGroup>,
    lt_arena: GPUArena<LocalTransform>,
}

impl LocalTransformBindGroup {
    #[allow(unused)]
    pub(super) fn get_first_buffer(&self) -> &wgpu::Buffer {
        self.lt_arena.get_first_buffer()
    }
    pub(in crate::renderer) fn write_lt_anim_data(
        &mut self,
        handle: &GPUInstanceHandle,
        lt_data: &[u8],
        queue: &wgpu::Queue,
    ) {
        let buf = self.lt_arena.get_first_buffer();
        let offset = self.lt_arena.resolve_byte_offset(handle) as u64;
        queue.write_buffer(buf, offset, lt_data);
    }

    pub(in crate::renderer) fn get_first_bg(&self) -> &wgpu::BindGroup {
        &self.bind_groups[0]
    }

    pub(super) fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let upload_result = self.lt_arena.upload(job, queue, device);
        if self.bind_groups.is_empty() {
            self.add_bind_group(device);
        }
        upload_result
    }
}

impl SharedInstanceBindGroup for LocalTransformBindGroup {
    fn register_shared_binding(
        &mut self,
        handle: &GPUInstanceHandle,
    ) -> Result<InstanceAllocationResult, AllocationTableError> {
        self.lt_arena.register_shared_binding(handle)
    }

    fn register_copy_binding(
        &mut self,
        handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<InstanceAllocationResult, AllocationTableError> {
        self.lt_arena.register_copy_binding(handle, queue, device)
    }
    fn release_prototype(
        &mut self,
        prototype: &crate::renderer::PrototypeHandle,
    ) -> Result<(), AllocationTableError> {
        self.lt_arena
            .remove_prototype_binding(prototype)
            .map_err(|_| AllocationTableError::PrototypeReleaseFailed)?;
        Ok(())
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
        let lts = GPUArena::<LocalTransform>::new();
        Self {
            bind_groups: vec![],
            lt_arena: lts,
        }
    }

    fn get_bind_group(&self, _handle: &InstanceHandle) -> &wgpu::BindGroup {
        // TODO: resolve based on alloc handle
        &self.bind_groups[0]
    }

    fn despawn(&mut self, handle: &GPUInstanceHandle) {
        let _ = self.lt_arena.dealloc(handle);
    }
}
