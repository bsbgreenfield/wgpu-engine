use std::{num::NonZero, range::Range};

use wgpu::{BindGroupEntry, BufferBinding};

use crate::{
    renderer::{
        AllocationMask, GPUAllocationHandle,
        bind_groups::BindGroupProvider,
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

    pub(in crate::renderer) fn resolve(
        &self,
        alloc_handle: &GPUAllocationHandle,
    ) -> (Range<u32>, &wgpu::Buffer) {
        self.material_arena.resolve(alloc_handle)
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
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<GPUUploadResult, VertexArenaError> {
        match self.texture_arena.upload(job, device, queue) {
            GPUUploadResult::Success => {}
            GPUUploadResult::TextureUploadBGDirty => {
                self.update_bind_group(device, 0); //TODO: find a way to get the actual bind group
            }
        }
        Ok(GPUUploadResult::Success)
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

        self.texture_arena.ensure_default(device, queue);
        self.material_arena.ensure_initialized(queue, device);
        self.add_bind_group(device);
    }

    pub(in crate::renderer) fn unload(
        &mut self,
        alloc_handle: &GPUAllocationHandle,
    ) -> Result<(), VertexArenaError> {
        self.material_arena.dealloc(alloc_handle)?;
        if alloc_handle.alloc_mask.contains(AllocationMask::TEX) {
            self.texture_arena.unload(alloc_handle)?;
        }

        Ok(())
    }

    pub(in crate::renderer) fn unload_texture(
        &mut self,
        alloc_handle: &GPUAllocationHandle,
    ) -> Result<(), VertexArenaError> {
        self.texture_arena.unload(alloc_handle)?;
        Ok(())
    }

    pub(in crate::renderer) fn get_default_bg(&self) -> &wgpu::BindGroup {
        &self.bind_groups[0]
    }

    fn get_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        let mut entries: Vec<BindGroupEntry> = (0..6)
            .into_iter()
            .map(|idx| wgpu::BindGroupEntry {
                binding: idx,
                resource: wgpu::BindingResource::TextureView(
                    self.texture_arena.get_view(idx as usize),
                ),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: 6,
            resource: wgpu::BindingResource::Sampler(&self.samplers[0]), // TODO: get actual
                                                                         // sampler
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 7,
            resource: wgpu::BindingResource::Buffer(BufferBinding {
                buffer: self.material_arena.get_first_buffer(),
                offset: 0,
                size: None,
            }),
        });
        let bgl = Self::get_bind_group_layout(device);
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture bind group"),
            layout: &bgl,
            entries: &entries,
        })
    }
}

impl BindGroupProvider for MaterialBindGroup {
    fn get_bind_group(
        &self,
        _alloc_handle: &crate::common::instance::InstanceHandle,
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
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

    fn update_bind_group(&mut self, device: &wgpu::Device, bg_idx: usize) {
        let new_bg = self.get_bind_group(device);
        self.bind_groups[bg_idx] = new_bg;
    }
    fn add_bind_group(&mut self, device: &wgpu::Device) {
        let bg = self.get_bind_group(device);
        self.bind_groups.push(bg);
    }

    fn new() -> Self {
        Self {
            bind_groups: Vec::new(),
            samplers: vec![],
            material_arena: GPUArena::<GPUMaterialData>::new(),
            texture_arena: TextureArena::new(),
            defaults_ready: false,
        }
    }

    #[allow(unused)]
    fn despawn(&mut self, handle: &crate::renderer::GPUInstanceHandle) {
        todo!()
    }
}
