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

#[allow(unused)]
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
    fn new(layer_count: u32) -> Self {
        Self {
            free_layers: Vec::from_iter(0..layer_count as usize),
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
            allocator: TextureAllocator::new(1),
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
            allocator: TextureAllocator::new(NUM_LAYERS),
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
    chunks: [Option<TextureChunk>; 6],
    alloc_table: TextureAllocTable,
}

impl TextureArena {
    pub fn new() -> Self {
        Self {
            chunks: [None, None, None, None, None, None],
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
            TexDim::Dim2048 => 5,
        }
    }

    pub fn ensure_default(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
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
    }

    fn ensure_chunk(&mut self, dimension: TexDim, device: &wgpu::Device) -> ChunkResult {
        let chunk_idx = Self::idx_from_tex_dim(dimension);
        if self.chunks[chunk_idx].is_none() {
            self.chunks[chunk_idx] = Some(TextureChunk::new(
                device,
                TextureFormat::Rgba8Unorm,
                dimension,
            ));
            return ChunkResult::New(chunk_idx);
        }
        return ChunkResult::Existing(chunk_idx);
    }
    pub fn upload(
        &mut self,
        job: UploadTextureJob,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> GPUUploadResult {
        let res = self.ensure_chunk(job.dim, device);
        let (ChunkResult::New(chunk_idx) | ChunkResult::Existing(chunk_idx)) = res;

        let chunk = self.chunks[chunk_idx]
            .as_mut()
            .expect("chunks should be initialized");
        match chunk.gpu_alloc(job.pixels, job.dim, queue) {
            Ok(layer) => {
                self.alloc_table.allocate(
                    GPUTextureHandle::new(job.texture_handle, 0),
                    AssetAllocationMeta::new(chunk_idx, layer),
                );
            }
            Err(e) => {
                panic!("texture upload fail {:?}", e)
            }
        }

        match res {
            ChunkResult::New(_) => GPUUploadResult::TextureUploadBGDirty,
            ChunkResult::Existing(_) => GPUUploadResult::Success,
        }
    }

    pub fn get_view(&self, chunk_idx: usize) -> &wgpu::TextureView {
        if let Some(chunk) = &self.chunks[chunk_idx] {
            &chunk.view
        } else {
            &self.chunks[0].as_ref().unwrap().view
        }
    }
}
enum ChunkResult {
    New(usize),
    Existing(usize),
}
