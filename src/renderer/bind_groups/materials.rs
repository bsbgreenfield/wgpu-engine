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
        let a = bytemuck::cast_slice::<u8, GPUMaterialData>(job.data);
        println!("JOB: {:?}", a);
        println!("BUCKET 1 and layer 0 equals {}", (1 << 16) | 0);
        let res = self.material_arena.upload(job, queue, device);
        res
    }

    pub(in crate::renderer) fn resolve_texture_slot(
        &self,
        alloc_handle: &GPUAllocationHandle,
        alloc_index: usize,
    ) -> Option<(u32, u32)> {
        self.texture_arena.resolve(alloc_handle, alloc_index)
    }

    pub(in crate::renderer) fn upload_texture(
        &mut self,
        job: UploadTextureJob,
        queue: &wgpu::Queue,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        let upload_result = self.texture_arena.upload(job, queue);
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

        self.texture_arena.ensure_chunks(device, queue);
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
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
        let view = self.texture_arena.get_views();
        let mut entries: Vec<wgpu::BindGroupEntry> = view
            .iter()
            .enumerate()
            .map(|(idx, view)| wgpu::BindGroupEntry {
                binding: idx as u32,
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect();

        entries.push(wgpu::BindGroupEntry {
            binding: 5,
            resource: wgpu::BindingResource::Sampler(&self.samplers[0]), // TODO: get actual
                                                                         // sampler
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 6,
            resource: wgpu::BindingResource::Buffer(BufferBinding {
                buffer: self.material_arena.get_first_buffer(),
                offset: 0,
                size: None,
            }),
        });
        let bgl = Self::get_bind_group_layout(device);
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture bind group"),
            layout: &bgl,
            entries: &entries,
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
