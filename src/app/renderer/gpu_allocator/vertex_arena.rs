use std::{
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, Range},
};

use crate::{
    app::renderer::{
        GPUAllocationHandle,
        gpu_allocator::{
            AllocMetaData, CHUNK_SIZE, GPUAllocator, GPUChunk, MIMIMUM_INDEX_ALLOCATION_SIZE,
            MIMIMUM_VERTEX_ALLOCATION_SIZE, UploadIndexJob, UploadMeshJob, VertexArenaError,
            free_list::FreeListAllocator,
        },
    },
    util::types::{GlobalTransform, ModelVertex, VIndex},
};
//****************************************************************
//
#[allow(unused)]
pub struct GPUArena<T: bytemuck::Pod + Debug> {
    max_chunks: usize,
    chunks: Vec<GPUChunk<T>>,
    alloc_table: HashMap<u32, AllocMetaData>,
    label: Option<String>,
    bind_group_layout: Option<wgpu::BindGroupLayout>,
}

impl GPUChunk<VIndex> {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            remaining_space: CHUNK_SIZE,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Index Buffer (u16)"),
                size: CHUNK_SIZE as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            allocator: FreeListAllocator::new(MIMIMUM_INDEX_ALLOCATION_SIZE),
            _t: PhantomData,
        }
    }
}

impl<T: ModelVertex> GPUChunk<T> {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            remaining_space: CHUNK_SIZE,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(format!("Vertex Buffer for {:?}", T::debug_str()).as_str()),
                size: CHUNK_SIZE as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            allocator: FreeListAllocator::new(MIMIMUM_VERTEX_ALLOCATION_SIZE),
            _t: PhantomData,
        }
    }
}

impl GPUAllocator<VIndex> for GPUArena<VIndex> {
    type UploadJob<'a> = UploadIndexJob<'a>;
    type AllocationError = VertexArenaError;

    fn new(device: &wgpu::Device) -> Self {
        Self {
            max_chunks: 16,
            chunks: vec![GPUChunk::<VIndex>::new(device)],
            alloc_table: HashMap::new(),
            label: Some(String::from("Index arena allocator")),
            bind_group_layout: None,
        }
    }
    fn upload<'a>(
        &mut self,
        job: Self::UploadJob<'a>,
        queue: &wgpu::Queue,
    ) -> Result<(), Self::AllocationError> {
        'outer: for (chunk_idx, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(job.indices, queue, self.label.as_ref().unwrap()) {
                Ok((node_idx, _)) => {
                    self.alloc_table
                        .insert(job.global_alloc_id, AllocMetaData::new(chunk_idx, node_idx));
                    return Ok(());
                }

                Err(e) => match e {
                    VertexArenaError::DataTooLarge(_, _) => {
                        return Err(e);
                    }
                    _ => continue 'outer,
                },
            }
        }
        Err(VertexArenaError::MaxAllocationReached)
    }

    // fn buffer_from_chunk_id(&self, chunk_id: usize) -> &wgpu::Buffer {
    //     &self.chunks[chunk_id].buffer
    // }
    // fn chunk_id(&self, handle: &GPUAllocationHandle) -> usize {
    //     self.alloc_table[&handle.global_allocation_id].chunk_id
    // }
    fn resolve(&self, handle: &GPUAllocationHandle) -> (Range<u32>, &wgpu::Buffer) {
        let meta = self.alloc_table.get(&handle.global_allocation_id).unwrap();
        let mut range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start = range.start / size_of::<VIndex>() as u32;
        range.end = range.end / size_of::<VIndex>() as u32;
        (range, &self.chunks[meta.chunk_id].buffer)
    }
}
impl<V: ModelVertex> GPUAllocator<V> for GPUArena<V> {
    type UploadJob<'a> = UploadMeshJob<'a, V>;
    type AllocationError = VertexArenaError;

    fn new(device: &wgpu::Device) -> Self {
        Self {
            bind_group_layout: None,
            max_chunks: 16,
            chunks: vec![GPUChunk::<V>::new(device)],
            alloc_table: HashMap::new(),
            label: Some(format!("Arena Allocator for {:?}", V::debug_str())),
        }
    }

    fn upload<'a>(
        &mut self,
        job: Self::UploadJob<'a>,
        queue: &wgpu::Queue,
    ) -> Result<(), Self::AllocationError> {
        //TODO fix these errors so they make sense
        'outer: for (chunk_idx, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(job.verts, queue, self.label.as_ref().unwrap()) {
                Ok((node_idx, _)) => {
                    self.alloc_table
                        .insert(job.global_alloc_id, AllocMetaData::new(chunk_idx, node_idx));
                    return Ok(());
                }

                Err(e) => match e {
                    VertexArenaError::DataTooLarge(_, _) => {
                        return Err(e);
                    }
                    _ => continue 'outer,
                },
            }
        }
        Err(VertexArenaError::MaxAllocationReached)
    }

    fn resolve(&self, handle: &GPUAllocationHandle) -> (Range<u32>, &wgpu::Buffer) {
        let meta = self.alloc_table.get(&handle.global_allocation_id).unwrap();
        let mut range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start = range.start / size_of::<V>() as u32;
        range.end = range.end / size_of::<V>() as u32;
        (range, &self.chunks[meta.chunk_id].buffer)
    }

    //  #[inline]
    //  fn chunk_id(&self, handle: &GPUAllocationHandle) -> usize {
    //      self.alloc_table[&handle.global_allocation_id].chunk_id
    //  }

    //  fn buffer_from_chunk_id(&self, chunk_id: usize) -> &wgpu::Buffer {
    //      &self.chunks[chunk_id].buffer
    //  }
}

pub struct StaticGPUBuffer<T: bytemuck::Pod> {
    _t: PhantomData<T>,
    buffer: wgpu::Buffer,
}

impl StaticGPUBuffer<GlobalTransform> {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            _t: PhantomData,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("global transform buffer"),
                size: 1677717,
                mapped_at_creation: false,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }),
        }
    }
}

impl<T: bytemuck::Pod> Deref for StaticGPUBuffer<T> {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
