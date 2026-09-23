use std::fmt::Display;

use wgpu::TextureFormat;

use crate::renderer::{
    GPUAllocationHandle, TexDim,
    gpu_allocator::{
        GPUUploadResult, UploadTextureJob, VertexArenaError,
        allocation_tables::{
            AllocationSlot, TAllocationTable,
            asset_alloc_table::AssetAllocationMeta,
            texture_alloc_table::{GPUTextureHandle, TextureAllocTable},
        },
    },
};

#[derive(Debug)]
enum TextureAllocationError {
    TextureWriteFailed,
    NoLayersLeft,
}

impl Display for TextureAllocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureAllocationError::TextureWriteFailed => {
                write!(f, "Texture could not be allocated")
            }
            TextureAllocationError::NoLayersLeft => write!(f, "This chunk is full"),
        }
    }
}

impl std::error::Error for TextureAllocationError {}

const NUM_LAYERS: u32 = 16;
struct TextureAllocator {
    free_layers: Vec<usize>,
}

impl TextureAllocator {
    fn new() -> Self {
        let fl = Vec::from_iter(0..NUM_LAYERS);
        assert!(fl.first() == Some(&0) && fl.last() == Some(&15));
        Self {
            free_layers: Vec::from_iter(0..NUM_LAYERS as usize),
        }
    }
}

struct TextureChunk {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    allocator: TextureAllocator,
}

impl TextureChunk {
    fn white(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(format!("default white pixel texture").as_str()),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[TextureFormat::Rgba8Unorm, TextureFormat::Rgba8UnormSrgb],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        Self {
            texture,
            view,
            allocator: TextureAllocator::new(),
        }
    }
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, dimension: TexDim) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(format!("Texture chunk for dimension: {dimension:?}").as_str()),
            size: wgpu::Extent3d {
                width: dimension.as_u32(),
                height: dimension.as_u32(),
                depth_or_array_layers: NUM_LAYERS,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[TextureFormat::Rgba8Unorm, TextureFormat::Rgba8UnormSrgb],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        Self {
            texture,
            view,
            allocator: TextureAllocator::new(),
        }
    }

    fn gpu_alloc(
        &mut self,
        pixels: &[u8],
        dimension: TexDim,
        queue: &wgpu::Queue,
    ) -> Result<usize, TextureAllocationError> {
        let layer = self
            .allocator
            .free_layers
            .pop()
            .ok_or(TextureAllocationError::NoLayersLeft)?;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer as u32,
                },
                aspect: wgpu::TextureAspect::default(),
            },
            pixels.as_ref(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimension.as_u32()),
                rows_per_image: Some(dimension.as_u32()),
            },
            wgpu::Extent3d {
                width: dimension.as_u32(),
                height: dimension.as_u32(),
                depth_or_array_layers: 1,
            },
        );
        Ok(layer)
    }

    fn dealloc(&mut self, layer_id: usize) {
        self.allocator.free_layers.push(layer_id);
    }
}

pub struct TextureArena {
    chunks: [Option<TextureChunk>; 5],
    alloc_table: TextureAllocTable,
}

impl<'frame> UploadTextureJob<'frame> {
    fn new_chunk(&self, device: &wgpu::Device) -> TextureChunk {
        TextureChunk::new(device, TextureFormat::Rgba8Unorm, self.dim)
    }
}

impl TextureArena {
    pub fn new() -> Self {
        Self {
            chunks: [None, None, None, None, None],
            alloc_table: TextureAllocTable::new(),
        }
    }

    pub(in crate::renderer) fn unload(
        &mut self,
        alloc_handle: &GPUAllocationHandle,
    ) -> Result<(), VertexArenaError> {
        let meta_list = self
            .alloc_table
            .dealloc_all(alloc_handle)
            .map_err(|_| VertexArenaError::DeallocError)?;
        for meta in meta_list {
            self.chunks[meta.chunk()]
                .as_mut()
                .unwrap()
                .dealloc(meta.node());
        }

        Ok(())
    }

    pub(in crate::renderer) fn resolve(
        &self,
        alloc_handle: &GPUAllocationHandle,
        alloc_index: usize,
    ) -> Option<(u32, u32)> {
        let meta = self
            .alloc_table
            .resolve(&GPUTextureHandle::new(alloc_handle.clone(), alloc_index))?;
        Some((meta.chunk() as u32, meta.node() as u32))
    }
    const fn idx_from_tex_dim(dimension: TexDim) -> usize {
        match dimension {
            TexDim::Dim1 => 0,
            TexDim::Dim64 => 1,
            TexDim::Dim128 => 2,
            TexDim::Dim256 => 3,
            TexDim::Dim1024 => 4,
        }
    }

    pub fn ensure_chunks(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.chunks[0].is_none() {
            let white_chunk = TextureChunk::white(device);

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &white_chunk.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::default(),
                },
                &[255u8, 255, 255, 255],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            self.chunks[0] = Some(white_chunk)
        }

        for (maybe_chunk, dim) in self
            .chunks
            .iter_mut()
            .skip(1)
            .zip([64, 128, 256, 1024].into_iter())
        {
            if maybe_chunk.is_none() {
                let _ = maybe_chunk.insert(TextureChunk::new(
                    device,
                    TextureFormat::Rgba8Unorm,
                    TexDim::from_u32(dim),
                ));
            }
        }
    }
    pub fn upload(&mut self, job: UploadTextureJob, queue: &wgpu::Queue) -> GPUUploadResult {
        let chunk_idx = Self::idx_from_tex_dim(job.dim);
        let chunk = self.chunks[chunk_idx]
            .as_mut()
            .expect("chunks should be initialized");

        match chunk.gpu_alloc(job.pixels, job.dim, queue) {
            Ok(layer) => {
                self.alloc_table.allocate(
                    GPUTextureHandle::new(job.texture_handle, 0),
                    AssetAllocationMeta::new(chunk_idx, layer),
                );
                return GPUUploadResult::Success;
            }
            Err(_) => {
                panic!("texture upload fail")
            }
        }
    }

    pub fn get_views(&self) -> [&wgpu::TextureView; 5] {
        [
            &self.chunks[0].as_ref().unwrap().view,
            &self.chunks[1].as_ref().unwrap().view,
            &self.chunks[2].as_ref().unwrap().view,
            &self.chunks[3].as_ref().unwrap().view,
            &self.chunks[4].as_ref().unwrap().view,
        ]
    }
}
