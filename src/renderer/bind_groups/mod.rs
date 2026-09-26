use std::collections::HashMap;

use crate::{
    common::instance::InstanceHandle,
    renderer::{
        GPUInstanceHandle, InstanceUploadJob, PrototypeHandle,
        bind_groups::{
            instance_data::InstanceDataBindGroup, local_transforms::LocalTransformBindGroup,
            materials::MaterialBindGroup, skinning::SkinningBindGroup,
        },
        gpu_allocator::{
            GPUUploadResult, VertexArenaError, allocation_tables::AllocationTableError,
            gpu_arena::InstanceAllocationResult,
        },
    },
    util::types::{InverseBindMatrix, JointTransform, LocalTransform},
};

pub(super) mod instance_data;
pub(super) mod local_transforms;
pub(super) mod materials;
pub(super) mod skinning;

pub(super) trait SharedInstanceBindGroup {
    fn register_shared_binding(
        &mut self,
        handle: &GPUInstanceHandle,
    ) -> Result<InstanceAllocationResult, AllocationTableError>;
    fn register_copy_binding(
        &mut self,
        handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<InstanceAllocationResult, AllocationTableError>;

    fn release_prototype(
        &mut self,
        prototype: &PrototypeHandle,
    ) -> Result<(), AllocationTableError>;
}

pub(super) trait BindGroupProvider {
    #[allow(unused)]
    fn get_bind_group(&self, alloc_handle: &InstanceHandle) -> &wgpu::BindGroup;
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
    fn add_bind_group(&mut self, device: &wgpu::Device);
    fn update_bind_group(&mut self, device: &wgpu::Device, bg_idx: usize);
    fn new() -> Self;
    fn despawn(&mut self, handle: &GPUInstanceHandle);
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct BindSlots {
    slots: [u8; 8],
}
struct BindRegistry {
    bindings: Vec<BindSlots>,
    lookup: HashMap<BindSlots, u16>,
}

pub(super) struct BindGroupCollection {
    pub(super) local_transforms: LocalTransformBindGroup,
    pub(super) skinning: SkinningBindGroup,
    pub(super) instance_data: InstanceDataBindGroup,
    pub(super) material_bind_group: MaterialBindGroup,
    bind_registry: BindRegistry,
}

impl BindGroupCollection {
    #[cfg(test)]
    pub(super) fn get_lt_buffer(&self) -> &wgpu::Buffer {
        self.local_transforms.get_first_buffer()
    }
    #[cfg(test)]
    pub(super) fn get_joint_buffer(&self) -> (&wgpu::Buffer, &wgpu::Buffer) {
        self.skinning.get_first_buffers()
    }

    pub(super) fn set_bindings(&mut self, bindings: [u8; 8]) -> u16 {
        let bs = BindSlots { slots: bindings };
        if let Some(id) = self.bind_registry.lookup.get(&bs) {
            return *id;
        } else {
            self.bind_registry
                .lookup
                .insert(bs.clone(), self.bind_registry.bindings.len() as u16);
            self.bind_registry.bindings.push(bs);
            (self.bind_registry.bindings.len() - 1) as u16
        }
    }

    pub(super) fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let res = self
            .local_transforms
            .upload_local_transforms(job, queue, device)?;

        Ok(res)
    }

    pub(super) fn upload_skin_data<'frame>(
        &mut self,
        joint_job: InstanceUploadJob<'frame, JointTransform>,
        ibm_job: InstanceUploadJob<'frame, InverseBindMatrix>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let res = self.skinning.upload(joint_job, ibm_job, queue, device)?;
        Ok(res)
    }

    pub(super) fn gen_gpu_instance_handle(
        &mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        prototype: &PrototypeHandle,
    ) -> Result<(u32, GPUInstanceHandle), AllocationTableError> {
        let (element_offset, node_id) = self.instance_data.reserve_record_slot(queue, device)?;
        Ok((
            node_id as u32,
            GPUInstanceHandle {
                instance_id: element_offset as u32,
                prototype: prototype.clone(),
                bind_id: u16::MAX,
            },
        ))
    }

    pub(super) fn new() -> Self {
        Self {
            local_transforms: LocalTransformBindGroup::new(),
            skinning: SkinningBindGroup::new(),
            instance_data: InstanceDataBindGroup::new(),
            material_bind_group: MaterialBindGroup::new(),
            bind_registry: BindRegistry {
                bindings: Vec::new(),
                lookup: HashMap::new(),
            },
        }
    }
    pub(super) fn despawn(&mut self, handle: &GPUInstanceHandle) {
        //if entry.ref_count == 0 {
        //    self.prototypes.remove(&handle.prototype);
        //}

        self.instance_data.despawn(handle);
        self.local_transforms.despawn(handle);
        self.skinning.despawn(handle);
    }

    pub(super) fn release_prototypes(
        &mut self,
        prototype: &PrototypeHandle,
    ) -> Result<(), AllocationTableError> {
        self.local_transforms.release_prototype(prototype)?;
        self.skinning.release_prototype(prototype)?;
        Ok(())
    }
}
