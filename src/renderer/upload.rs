use crate::{
    renderer::{
        InstanceUploadJob,
        gpu_allocator::{
            GPUAllocator, GPUUploadResult, UploadIndexJob, UploadMaterialJob, UploadTextureJob,
            VertexArenaError,
        },
        renderer::Renderer,
    },
    util::types::{InstanceRecordData, InverseBindMatrix, JointTransform, LocalTransform},
};

impl Renderer {
    pub(super) fn upload_texture<'frame>(
        &mut self,
        job: UploadTextureJob,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        self.bind_groups
            .material_bind_group
            .upload_texture(job, device, queue)?;
        Ok(())
    }

    pub(super) fn upload_materials<'frame>(
        &mut self,
        job: UploadMaterialJob,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError> {
        self.bind_groups
            .material_bind_group
            .upload_materials(job, queue, device)?;
        Ok(())
    }

    pub(super) fn upload_indices_16<'frame>(
        &mut self,
        job: UploadIndexJob,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError> {
        self.vertex_arenas
            .index_arena_16
            .upload(job, queue, device)?;
        Ok(())
    }
    pub(super) fn upload_indices_32<'frame>(
        &mut self,
        job: UploadIndexJob,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError> {
        self.vertex_arenas
            .index_arena_32
            .upload(job, queue, device)?;
        Ok(())
    }

    pub(super) fn upload_instance_record<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, InstanceRecordData>,
        node_id: u32,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        Ok(self
            .bind_groups
            .instance_data
            .upload_instance_record(job, node_id, queue, device)?)
    }

    pub(super) fn upload_local_transforms<'frame>(
        &mut self,
        job: InstanceUploadJob<'frame, LocalTransform>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        self.bind_groups.upload_local_transforms(job, queue, device)
    }

    pub(super) fn upload_skin_data<'frame>(
        &mut self,
        joint_job: InstanceUploadJob<'frame, JointTransform>,
        ibm_job: InstanceUploadJob<'frame, InverseBindMatrix>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        self.bind_groups
            .upload_skin_data(joint_job, ibm_job, queue, device)
    }
}
