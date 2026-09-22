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
    fn new() -> Self;
    fn despawn(&mut self, handle: &GPUInstanceHandle);
}

pub(super) struct BindGroupCollection {
    next_handle: u32,
    pub(super) local_transforms: LocalTransformBindGroup,
    pub(super) skinning: SkinningBindGroup,
    pub(super) instance_data: InstanceDataBindGroup,
    pub(super) material_bind_group: MaterialBindGroup,
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

    pub(super) fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let res = self
            .local_transforms
            .upload_local_transforms(job, queue, device)?;
        if let GPUUploadResult::PrototypeUploaded = res {
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
        let res = self.skinning.upload(joint_job, ibm_job, queue, device)?;

        let GPUUploadResult::PrototypeUploaded = res else {
            panic!("wrong upload type");
        };
        Ok(res)
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

    pub(super) fn new() -> Self {
        Self {
            next_handle: 0,
            local_transforms: LocalTransformBindGroup::new(),
            skinning: SkinningBindGroup::new(),
            instance_data: InstanceDataBindGroup::new(),
            material_bind_group: MaterialBindGroup::new(),
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
