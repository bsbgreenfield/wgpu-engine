use std::collections::HashMap;

use crate::{
    common::instance::InstanceHandle,
    renderer::{
        BufferType, GPUInstanceHandle, InstanceUploadJob, PrototypeHandle, StorageData,
        bind_groups::{
            instance_data::InstanceDataBindGroup, local_transforms::LocalTransformBindGroup,
            skinning::SkinningBindGroup,
        },
        gpu_allocator::{GPUUploadResult, VertexArenaError},
    },
    util::types::{InverseBindMatrix, JointTransform, LocalTransform},
};

pub(super) mod instance_data;
pub(super) mod local_transforms;
pub(super) mod skinning;

pub(super) trait BindGroupProvider {
    fn get_bind_group(&self, alloc_handle: &InstanceHandle) -> &wgpu::BindGroup;
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
    fn add_bind_group(&mut self, device: &wgpu::Device);
    fn new() -> Self;
    fn despawn(&mut self, handle: &GPUInstanceHandle);
}

pub(super) trait SharedInstanceData: StorageData {}

#[derive(Debug)]
struct PrototypeEntry {
    ref_count: usize,
    local_transforms_slot: usize,
    joint_transforms_slot: Option<usize>,
}

pub(super) struct BindGroupCollection {
    next_handle: u32,
    prototypes: HashMap<PrototypeHandle, PrototypeEntry>,
    pub(super) local_transforms: LocalTransformBindGroup,
    pub(super) skinning: SkinningBindGroup,
    pub(super) instance_data: InstanceDataBindGroup,
}

impl BindGroupCollection {
    #[cfg(test)]
    pub(super) fn get_prototype_count(&self) -> usize {
        self.prototypes.len()
    }
    #[cfg(test)]
    pub(super) fn get_prototype_ref_count(&self, handle: &PrototypeHandle) -> Option<usize> {
        self.prototypes.get(handle).map(|p| p.ref_count)
    }

    #[cfg(test)]
    pub(super) fn get_lt_buffer(&self) -> &wgpu::Buffer {
        self.local_transforms.get_first_buffer()
    }
    #[cfg(test)]
    pub(super) fn get_joint_buffer(&self) -> (&wgpu::Buffer, &wgpu::Buffer) {
        self.skinning.get_first_buffers()
    }

    pub(super) fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let prototype = job.gpu_instance_handle.prototype.clone();
        let res = self
            .local_transforms
            .upload_local_transforms(job, queue, device)?;
        if let GPUUploadResult::BindGroupUploadResult {
            buffer_element_offset: _,
            alloc_meta_idx,
        } = res
        {
            self.prototypes
                .entry(prototype)
                .and_modify(|entry| entry.local_transforms_slot = alloc_meta_idx);
        } else {
            panic!("wrong upload type");
        }

        Ok(res)
    }

    pub(super) fn upload_skin_data<'frame>(
        &mut self,
        joint_job: InstanceUploadJob<'frame, JointTransform>,
        ibm_job: InstanceUploadJob<'frame, InverseBindMatrix>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let prototype = joint_job.gpu_instance_handle.prototype.clone();
        let res = self.skinning.upload(joint_job, ibm_job, queue, device)?;

        let GPUUploadResult::BindGroupUploadResult {
            buffer_element_offset: _,
            alloc_meta_idx,
        } = res
        else {
            panic!("wrong upload type");
        };
        self.prototypes
            .entry(prototype)
            .and_modify(|entry| entry.joint_transforms_slot = Some(alloc_meta_idx));
        Ok(res)
    }

    pub(super) fn get_slot(&self, prototype: &PrototypeHandle, buffer_type: BufferType) -> usize {
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
    pub(super) fn gen_gpu_instance_handle(
        &mut self,
        prototype: &PrototypeHandle,
    ) -> GPUInstanceHandle {
        self.next_handle += 1;
        GPUInstanceHandle {
            instance_id: self.next_handle - 1,
            prototype: prototype.clone(),
        }
    }

    pub(super) fn add_prototype(&mut self, prototype: PrototypeHandle) {
        self.prototypes.insert(
            prototype,
            PrototypeEntry {
                ref_count: 0,
                local_transforms_slot: 0, //dummy temp
                joint_transforms_slot: None,
            },
        );
    }
    pub(super) fn add_prototype_instance(&mut self, prototype: &PrototypeHandle) {
        self.prototypes
            .get_mut(prototype)
            .expect("should be prototype")
            .ref_count += 1;
    }

    pub(super) fn new() -> Self {
        Self {
            next_handle: 0,
            prototypes: HashMap::new(),
            local_transforms: LocalTransformBindGroup::new(),
            skinning: SkinningBindGroup::new(),
            instance_data: InstanceDataBindGroup::new(),
        }
    }
    pub(super) fn despawn(&mut self, handle: &GPUInstanceHandle) {
        let prototype_entry = self
            .prototypes
            .get_mut(&handle.prototype)
            .expect("invalid instance");

        prototype_entry.ref_count -= 1;

        //if entry.ref_count == 0 {
        //    self.prototypes.remove(&handle.prototype);
        //}

        self.instance_data.despawn(handle);
        self.local_transforms.despawn(handle);
        self.skinning.despawn(handle);
    }
}
