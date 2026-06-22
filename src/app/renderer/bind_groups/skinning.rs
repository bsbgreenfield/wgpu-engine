use std::num::NonZero;

use wgpu::ShaderStages;

use crate::{
    app::renderer::{
        InstanceUploadJob,
        bind_groups::{BindGroupProvider, BindGroupUploadResult, SharedInstanceData},
        gpu_allocator::{GPUInstanceAllocator, VertexArenaError, instance_arena::InstanceArena},
        renderer::GPUInstanceHandle,
    },
    util::types::{InverseBindMatrix, JointTransform, Mat4F32},
    world::instance_manager::InstanceHandle,
};

pub struct SkinningBindGroup {
    bind_groups: Vec<wgpu::BindGroup>,
    joint_arena: InstanceArena<JointTransform>,
    ibm_arena: InstanceArena<InverseBindMatrix>,
}

impl SkinningBindGroup {
    pub fn write_joint_anim_data(
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
    pub fn upload<'frame>(
        &mut self,
        joint_job: InstanceUploadJob<'frame, JointTransform>,
        ibm_job: InstanceUploadJob<'frame, InverseBindMatrix>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<BindGroupUploadResult, VertexArenaError> {
        let jt_result = self.joint_arena.upload(joint_job, queue, device)?;
        let _ibm_offset = self.ibm_arena.upload(ibm_job, queue, device)?;
        if self.bind_groups.is_empty() {
            self.add_bind_group(device);
        }
        Ok(jt_result)
    }

    pub fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<(u32, u32), VertexArenaError> {
        let jt = self
            .joint_arena
            .register_shared_binding(slot_index, new_handle);
        let ibm = self
            .ibm_arena
            .register_shared_binding(slot_index, new_handle);

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

    pub fn register_copy_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(u32, u32), VertexArenaError> {
        let jt = self
            .joint_arena
            .register_copy_binding(slot_index, new_handle, queue, device);
        let ibm = self
            .ibm_arena
            .register_shared_binding(slot_index, new_handle);
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

    pub fn get_first_bg(&self) -> &wgpu::BindGroup {
        &self.bind_groups[0]
    }
}
impl BindGroupProvider for SkinningBindGroup {
    fn add_bind_group(&mut self, device: &wgpu::Device) {
        let bgl = Self::get_bind_group_layout(device);
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skinning Bind Group"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: self.joint_arena.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: self.ibm_arena.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });
        self.bind_groups.push(bg);
    }
    fn new() -> Self {
        let joints = InstanceArena::<JointTransform>::new();
        let ibms = InstanceArena::<InverseBindMatrix>::new();
        Self {
            bind_groups: vec![],
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

    fn despawn(&mut self, handle: &GPUInstanceHandle) {
        let _ = self.joint_arena.remove(handle);
        let _ = self.ibm_arena.remove(handle);
    }
}
