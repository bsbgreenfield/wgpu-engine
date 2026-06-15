use std::num::NonZero;

use wgpu::ShaderStages;

use crate::{
    app::renderer::{
        InstanceUploadJob,
        gpu_allocator::{
            GPUInstanceAllocator, SharedInstanceData, VertexArenaError,
            instance_arena::InstanceArena,
        },
        renderer::GPUInstanceHandle,
    },
    util::types::{
        InstanceOffset, InstanceRecordData, InverseBindMatrix, JointTransform, LocalTransform,
        Mat4F32,
    },
    world::instance_manager::{InstanceGPUBindings, InstanceHandle, RenderFrame},
};

pub(super) trait BindGroupProvider {
    fn get_bind_group(&self, alloc_handle: &InstanceHandle) -> &wgpu::BindGroup;
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
    fn new(device: &wgpu::Device) -> Self;
}

impl BindGroupProvider for SkinningBindGroup {
    fn new(device: &wgpu::Device) -> Self {
        let joints = InstanceArena::<JointTransform>::new(device);
        let ibms = InstanceArena::<InverseBindMatrix>::new(device);
        let bgl = Self::get_bind_group_layout(device);
        let initial_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skinning Bind Group"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: joints.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: ibms.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });
        Self {
            bind_groups: vec![initial_bind_group],
            _bind_group_layout: bgl,
            joint_arena: joints,
            ibm_arena: ibms,
        }
    }
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skinning bind group"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::<u64>::new(size_of::<Mat4F32>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::<u64>::new(size_of::<Mat4F32>() as u64),
                    },
                    count: None,
                },
            ],
        })
    }

    fn get_bind_group(&self, _handle: &InstanceHandle) -> &wgpu::BindGroup {
        // this should be like
        // let joint_chunk_idx = self.joints.resolve(handle)
        // let ibm chunk_idx = self.ibms.resolve(handle)
        // self.get_bind_group(joint_chunk, ibm_chunk)
        &self.bind_groups[0]
    }
}

pub(super) struct SkinningBindGroup {
    _bind_group_layout: wgpu::BindGroupLayout,
    bind_groups: Vec<wgpu::BindGroup>,
    joint_arena: InstanceArena<JointTransform>,
    ibm_arena: InstanceArena<InverseBindMatrix>,
}

impl SkinningBindGroup {
    pub(super) fn write_joint_anim_data(
        &self,
        gpu_handle: &GPUInstanceHandle,
        joint_data: &[u8],
        queue: &wgpu::Queue,
    ) {
        let buffer = self.get_joint_buffer();
        let jt_offset = self.joint_arena.resolve(gpu_handle);
        queue.write_buffer(buffer, jt_offset.into(), joint_data);
    }

    pub(super) fn get_joint_buffer(&self) -> &wgpu::Buffer {
        //TODO: resolve using handle
        self.joint_arena.get_first_buffer()
    }
    pub(super) fn upload<'frame>(
        &mut self,
        joint_job: InstanceUploadJob<'frame, JointTransform>,
        ibm_job: InstanceUploadJob<'frame, InverseBindMatrix>,
        queue: &wgpu::Queue,
    ) -> Result<u32, VertexArenaError> {
        let jt_offset = self.joint_arena.upload(joint_job, queue)?;
        let _ibm_offset = self.ibm_arena.upload(ibm_job, queue)?;
        Ok(jt_offset)
    }

    pub(super) fn register_shared_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
    ) -> Result<(u32, u32), VertexArenaError> {
        let jt = self
            .joint_arena
            .register_shared_binding(donor_handle, new_handle);
        let ibm = self
            .ibm_arena
            .register_shared_binding(donor_handle, new_handle);

        if let Ok(jt) = jt {
            if let Ok(ibm) = ibm {
                return Ok((jt, ibm));
            } else {
                return Err(ibm.unwrap_err());
            }
        } else {
            return Err(jt.unwrap_err());
        }
    }

    pub(super) fn register_copy_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(u32, u32), VertexArenaError> {
        let jt = self
            .joint_arena
            .register_copy_binding(donor_handle, new_handle, queue, device);
        let ibm = self
            .ibm_arena
            .register_shared_binding(donor_handle, new_handle);
        if let Ok(jt) = jt {
            if let Ok(ibm) = ibm {
                return Ok((jt, ibm));
            } else {
                return Err(ibm.unwrap_err());
            }
        } else {
            return Err(jt.unwrap_err());
        }
    }

    pub(super) fn get_first_bg(&self) -> &wgpu::BindGroup {
        &self.bind_groups[0]
    }
}

impl BindGroupProvider for LocalTransformBindGroup {
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("LT bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::<u64>::new(size_of::<Mat4F32>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::<u64>::new(16),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::<u64>::new(4),
                    },
                    count: None,
                },
            ],
        })
    }

    fn new(device: &wgpu::Device) -> Self {
        let lts = InstanceArena::<LocalTransform>::new(device);
        let instance_records = InstanceArena::<InstanceRecordData>::new(device);
        let instance_offsets = InstanceArena::<InstanceOffset>::new(device);
        let bgl = Self::get_bind_group_layout(device);
        let initial_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lt bind group"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: lts.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: instance_records.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: instance_offsets.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });
        Self {
            bind_groups: vec![initial_bind_group],
            lt_arena: lts,
            record_arena: instance_records,
            offsets: instance_offsets,
        }
    }

    fn get_bind_group(&self, handle: &InstanceHandle) -> &wgpu::BindGroup {
        // TODO: resolve based on alloc handle
        &self.bind_groups[0]
    }
}

pub(super) struct LocalTransformBindGroup {
    bind_groups: Vec<wgpu::BindGroup>,
    lt_arena: InstanceArena<LocalTransform>,
    record_arena: InstanceArena<InstanceRecordData>,
    offsets: InstanceArena<InstanceOffset>,
}

impl LocalTransformBindGroup {
    pub(super) fn write_lt_anim_data(
        &self,
        handle: &GPUInstanceHandle,
        lt_data: &[u8],
        queue: &wgpu::Queue,
    ) {
        let buf = self.get_lt_buffer();
        let offset = self.lt_arena.resolve(handle) as u64;
        queue.write_buffer(buf, offset, lt_data);
    }
    pub(super) fn get_lt_buffer(&self) -> &wgpu::Buffer {
        self.lt_arena.get_first_buffer()
    }

    pub(super) fn get_first_bg(&self) -> &wgpu::BindGroup {
        &self.bind_groups[0]
    }

    pub(super) fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
    ) -> Result<u32, VertexArenaError> {
        self.lt_arena.upload(job, queue)
    }

    pub(super) fn upload_instance_record<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, InstanceRecordData>,
        queue: &wgpu::Queue,
    ) -> Result<u32, VertexArenaError> {
        self.record_arena.upload(job, queue)
    }
    pub(super) fn upload_instance_offsets<'frame>(
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

    pub(super) fn register_shared_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        self.lt_arena
            .register_shared_binding(donor_handle, new_handle)
    }

    pub(super) fn register_copy_binding(
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
