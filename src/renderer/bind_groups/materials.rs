use crate::{
    renderer::{
        GPUAllocationHandle,
        bind_groups::{BGBufferType, BindGroupProvider},
        gpu_allocator::{
            GPUAllocator, GPUUploadResult, UploadMaterialJob, UploadTextureJob, VertexArenaError,
            gpu_arena::GPUArena, texture_arena::TextureArena,
        },
    },
    util::types::GPUMaterialData,
};

pub(in crate::renderer) struct MaterialBindGroup {
    bind_groups: Vec<wgpu::BindGroup>,
    samplers: Vec<wgpu::Sampler>,
    material_arena: GPUArena<GPUMaterialData>,
    texture_arena: TextureArena,
}

impl MaterialBindGroup {
    pub(in crate::renderer) fn upload_materials(
        &mut self,
        job: UploadMaterialJob,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        self.material_arena.upload(job, queue, device)
    }

    pub(in crate::renderer) fn upload_texture(
        &mut self,
        job: UploadTextureJob,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        Ok(self.texture_arena.upload(job, queue, device))
    }

    pub(in crate::renderer) fn unload(
        &mut self,
        alloc_handle: &GPUAllocationHandle,
    ) -> Result<(), VertexArenaError> {
        // TODO:
        Ok(())
    }
}

impl BindGroupProvider for MaterialBindGroup {
    fn get_bind_group(
        &self,
        alloc_handle: &crate::common::instance::InstanceHandle,
    ) -> &wgpu::BindGroup {
        todo!()
    }

    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material bind group"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // TODO: material
            ],
        })
    }

    fn add_bind_group(&mut self, device: &wgpu::Device, ty: BGBufferType) {
        if self.samplers.is_empty() {
            self.samplers
                .push(device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("default sampler"),
                    ..Default::default()
                }));
        }
        let bgl = Self::get_bind_group_layout(device);
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture bind group"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &self.texture_arena.get_texture_ref(ty),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.samplers[0]), // TODO: get actual
                                                                                 // sampler
                },
            ],
        });
        self.bind_groups.push(bg);
    }

    fn new() -> Self {
        Self {
            bind_groups: Vec::new(),
            samplers: vec![],
            material_arena: GPUArena::<GPUMaterialData>::new(),
            texture_arena: TextureArena::new(),
        }
    }

    fn despawn(&mut self, handle: &crate::renderer::GPUInstanceHandle) {
        todo!()
    }
}
