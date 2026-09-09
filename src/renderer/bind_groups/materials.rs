use std::{collections::HashMap, num::NonZero};

use wgpu::BufferBinding;

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
    texture_type_map: HashMap<u32, usize>,
    bind_groups: Vec<wgpu::BindGroup>,
    samplers: Vec<wgpu::Sampler>,
    material_arena: GPUArena<GPUMaterialData>,
    texture_arena: TextureArena,
    defaults_ready: bool,
}

impl MaterialBindGroup {
    pub(in crate::renderer) fn upload_materials(
        &mut self,
        job: UploadMaterialJob,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let res = self.material_arena.upload(job, queue, device);
        res
    }

    pub(in crate::renderer) fn upload_texture(
        &mut self,
        job: UploadTextureJob,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let needs_bg: Option<u32> = if self.texture_type_map.get(&job.data.height).is_none() {
            Some(job.data.height)
        } else {
            None
        };
        let upload_result = self.texture_arena.upload(job, queue, device);
        if let Some(texture_dim) = needs_bg {
            let ty = BGBufferType::tex_dim_from_u32(texture_dim);
            self.add_bind_group(device, ty);
        }
        Ok(upload_result)
    }

    pub(in crate::renderer) fn ensure_defaults(
        &mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) {
        if self.defaults_ready {
            return;
        }

        self.defaults_ready = true;

        self.samplers
            .push(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("default sampler"),
                ..Default::default()
            }));

        self.texture_arena.upload_default(device, queue);
        self.material_arena.ensure_initialized(queue, device);
        self.add_bind_group(device, BGBufferType::Texture1);
    }

    pub(in crate::renderer) fn unload(
        &mut self,
        alloc_handle: &GPUAllocationHandle,
    ) -> Result<(), VertexArenaError> {
        // TODO:
        Ok(())
    }

    pub(in crate::renderer) fn get_default_bg(&self) -> &wgpu::BindGroup {
        &self.bind_groups[0]
    }
}

impl BindGroupProvider for MaterialBindGroup {
    fn get_bind_group(
        &self,
        alloc_handle: &crate::common::instance::InstanceHandle,
    ) -> &wgpu::BindGroup {
        &self.bind_groups[0]
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
                        view_dimension: wgpu::TextureViewDimension::D2Array,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::new(32),
                    },
                    count: None,
                },
            ],
        })
    }

    fn add_bind_group(&mut self, device: &wgpu::Device, ty: BGBufferType) {
        println!("texture type is : {:?}", ty);

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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: self.material_arena.get_first_buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });
        let idx = self.bind_groups.len();
        self.bind_groups.push(bg);
        if let Some(dim) = ty.u32_from_dim() {
            self.texture_type_map.insert(dim, idx);
        }
    }

    fn new() -> Self {
        Self {
            bind_groups: Vec::new(),
            samplers: vec![],
            material_arena: GPUArena::<GPUMaterialData>::new(),
            texture_arena: TextureArena::new(),
            texture_type_map: HashMap::new(),
            defaults_ready: false,
        }
    }

    fn despawn(&mut self, handle: &crate::renderer::GPUInstanceHandle) {
        todo!()
    }
}
