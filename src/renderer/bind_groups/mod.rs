use std::collections::HashMap;

use crate::{
    common::instance::{GPUInstanceHandle, InstanceHandle},
    renderer::{
        BufferType, InstanceUploadJob, PrototypeHandle, StorageData,
        bind_groups::{
            instance_data::InstanceDataBindGroup, local_transforms::LocalTransformBindGroup,
            skinning::SkinningBindGroup,
        },
        gpu_allocator::VertexArenaError,
    },
    util::types::{InverseBindMatrix, JointTransform, LocalTransform},
};

pub mod instance_data;
pub mod local_transforms;
pub mod skinning;

pub trait BindGroupProvider {
    fn get_bind_group(&self, alloc_handle: &InstanceHandle) -> &wgpu::BindGroup;
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
    fn add_bind_group(&mut self, device: &wgpu::Device);
    fn new() -> Self;
    fn despawn(&mut self, handle: &GPUInstanceHandle);
}

pub trait SharedInstanceData: StorageData {}

#[derive(Debug)]
struct PrototypeEntry {
    pub(super) ref_count: usize,
    pub local_transforms_slot: usize,
    pub joint_transforms_slot: Option<usize>,
}

pub struct BindGroupUploadResult {
    pub buffer_offset: u32,
    pub alloc_meta_idx: usize,
}
pub struct BindGroupCollection {
    next_handle: u32,
    prototypes: HashMap<PrototypeHandle, PrototypeEntry>,
    pub local_transforms: LocalTransformBindGroup,
    pub skinning: SkinningBindGroup,
    pub instance_data: InstanceDataBindGroup,
}

impl BindGroupCollection {
    #[cfg(test)]
    pub fn get_prototype_count(&self) -> usize {
        self.prototypes.len()
    }
    #[cfg(test)]
    pub fn get_prototype_ref_count(&self, handle: &PrototypeHandle) -> Option<usize> {
        self.prototypes.get(handle).map(|p| p.ref_count)
    }

    pub fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        let prototype = job.gpu_instance_handle.prototype.clone();
        let upload_result = self
            .local_transforms
            .upload_local_transforms(job, queue, device)?;
        self.prototypes
            .entry(prototype)
            .and_modify(|entry| entry.local_transforms_slot = upload_result.alloc_meta_idx);
        Ok(upload_result.buffer_offset)
    }

    pub fn upload_skin_data<'frame>(
        &mut self,
        joint_job: InstanceUploadJob<'frame, JointTransform>,
        ibm_job: InstanceUploadJob<'frame, InverseBindMatrix>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        let prototype = joint_job.gpu_instance_handle.prototype.clone();
        let upload_result = self.skinning.upload(joint_job, ibm_job, queue, device)?;
        self.prototypes
            .entry(prototype)
            .and_modify(|entry| entry.joint_transforms_slot = Some(upload_result.alloc_meta_idx));
        Ok(upload_result.buffer_offset)
    }

    pub fn get_slot(&self, prototype: &PrototypeHandle, buffer_type: BufferType) -> usize {
        match buffer_type {
            BufferType::LocalTransform => {
                self.prototypes
                    .get(prototype)
                    .expect("prototype not registered")
                    .local_transforms_slot
            }
            BufferType::JointTransform => self
                .prototypes
                .get(prototype)
                .expect("prototype not registered")
                .joint_transforms_slot
                .expect("joints dont exist for prototype"),
        }
    }
    pub fn gen_gpu_instance_handle(&mut self, prototype: &PrototypeHandle) -> GPUInstanceHandle {
        self.next_handle += 1;
        GPUInstanceHandle {
            instance_id: self.next_handle - 1,
            prototype: prototype.clone(),
        }
    }

    pub fn add_prototype(&mut self, prototype: PrototypeHandle) {
        self.prototypes.insert(
            prototype,
            PrototypeEntry {
                ref_count: 0,
                local_transforms_slot: 0,
                joint_transforms_slot: None,
            },
        );
    }
    pub fn add_prototype_instance(&mut self, prototype: &PrototypeHandle) {
        self.prototypes
            .get_mut(prototype)
            .expect("should be prototype")
            .ref_count += 1;
    }

    pub fn new() -> Self {
        Self {
            next_handle: 0,
            prototypes: HashMap::new(),
            local_transforms: LocalTransformBindGroup::new(),
            skinning: SkinningBindGroup::new(),
            instance_data: InstanceDataBindGroup::new(),
        }
    }
    pub fn despawn(&mut self, handle: &GPUInstanceHandle) {
        let entry = self
            .prototypes
            .get_mut(&handle.prototype)
            .expect("invalid instance");

        entry.ref_count -= 1;

        //if entry.ref_count == 0 {
        //    self.prototypes.remove(&handle.prototype);
        //}

        self.instance_data.despawn(handle);
        self.local_transforms.despawn(handle);
        self.skinning.despawn(handle);
    }
}
