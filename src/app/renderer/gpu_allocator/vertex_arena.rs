use std::{
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    num::NonZero,
    ops::{Deref, Range},
};

use crate::{
    app::renderer::{
        GPUAllocationHandle,
        gpu_allocator::{
            CHUNK_SIZE, GPUAllocator, LocalTransformUploadJob, UploadMeshJob, VertexArenaError,
            free_list::FreeListAllocator,
        },
    },
    util::types::{GlobalTransform, LocalTransform, ModelVertex},
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

struct GPUChunk<T: bytemuck::Pod + Debug> {
    remaining_space: u32,
    buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    allocator: FreeListAllocator,
    _t: PhantomData<T>,
}

impl GPUChunk<LocalTransform> {
    fn new(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout) -> Self {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("local transform storage buffer"),
            size: CHUNK_SIZE as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let new_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lt bind group"),
            layout: bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        });
        Self {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            bind_group: Some(new_bg),
            buffer: buf,
            allocator: FreeListAllocator::new(),
            _t: PhantomData,
        }
    }
}

impl<T: bytemuck::Pod + Debug> GPUChunk<T> {
    fn gpu_alloc(
        &mut self,
        data: &[T],
        queue: &wgpu::Queue,
        label: &str,
    ) -> Result<(usize, Range<u32>), VertexArenaError> {
        let size = (data.len() * size_of::<T>()) as u32;
        let node_idx: usize = if self.remaining_space >= size {
            self.allocator.alloc_first(size)?
        } else {
            return Err(VertexArenaError::DataTooLarge(size, label.to_string()));
        };
        // for datum in data.iter().take(10) {
        //     println!("{:?}", datum);
        // }
        let offset = self.allocator.offset_of(node_idx) as u32;
        queue.write_buffer(&self.buffer, offset.into(), bytemuck::cast_slice(data));
        Ok((node_idx, offset..offset + (data.len() as u32)))
    }
}

impl<T: ModelVertex> GPUChunk<T> {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            remaining_space: CHUNK_SIZE,
            bind_group: None,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: CHUNK_SIZE as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            allocator: FreeListAllocator::new(),
            _t: PhantomData,
        }
    }
}

#[derive(Debug)]
struct AllocMetaData {
    chunk_id: usize,
    node_id: usize,
}
impl AllocMetaData {
    fn new(chunk_id: usize, node_id: usize) -> Self {
        Self { chunk_id, node_id }
    }
}

impl GPUAllocator<LocalTransform> for GPUArena<LocalTransform> {
    type UploadJob<'a> = LocalTransformUploadJob<'a>;
    type AllocationError = VertexArenaError;

    fn new(device: &wgpu::Device) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(" lt bind group LAYOUT"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                count: None,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(NonZero::new(64).unwrap()),
                },
                visibility: wgpu::ShaderStages::VERTEX,
            }],
        });
        Self {
            max_chunks: 1,
            chunks: vec![GPUChunk::<LocalTransform>::new(device, &bgl)],
            alloc_table: HashMap::new(),
            bind_group_layout: Some(bgl),
            label: Some("Local transform buffer".to_string()),
        }
    }
    fn upload<'a>(
        &mut self,
        job: Self::UploadJob<'a>,
        queue: &wgpu::Queue,
    ) -> Result<(), Self::AllocationError> {
        let (node_id, _range) =
            self.chunks[0].gpu_alloc(job.local_transforms, queue, self.label.as_ref().unwrap())?;
        self.alloc_table.insert(
            job.global_alloc_id,
            AllocMetaData {
                chunk_id: 0,
                node_id,
            },
        );

        Ok(())
    }

    fn resolve(
        &self,
        handle: &GPUAllocationHandle,
    ) -> (Range<u32>, &wgpu::Buffer, Option<&wgpu::BindGroup>) {
        let node_id = self
            .alloc_table
            .get(&handle.global_allocation_id)
            .unwrap()
            .node_id;

        let mut allocation_range = self.chunks[0].allocator.resolve(node_id);

        allocation_range.start = allocation_range.start / size_of::<LocalTransform>() as u32;
        allocation_range.end = allocation_range.end / size_of::<LocalTransform>() as u32;

        (
            allocation_range,
            &self.chunks[0].buffer,
            Some(self.get_bind_group()),
        )
    }

    fn chunk_id(&self, _: &GPUAllocationHandle) -> usize {
        0
    }

    fn buffer_from_chunk_id(&self, _: usize) -> &wgpu::Buffer {
        &self.chunks[0].buffer
    }
}

impl GPUArena<LocalTransform> {
    pub fn get_bind_group(&self) -> &wgpu::BindGroup {
        return self.chunks[0].bind_group.as_ref().unwrap();
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
            label: Some(V::debug_str()),
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

    fn resolve(
        &self,
        handle: &GPUAllocationHandle,
    ) -> (Range<u32>, &wgpu::Buffer, Option<&wgpu::BindGroup>) {
        let meta = self.alloc_table.get(&handle.global_allocation_id).unwrap();
        let mut range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start = range.start / size_of::<V>() as u32;
        range.end = range.end / size_of::<V>() as u32;
        (range, &self.chunks[meta.chunk_id].buffer, None)
    }

    #[inline]
    fn chunk_id(&self, handle: &GPUAllocationHandle) -> usize {
        self.alloc_table[&handle.global_allocation_id].chunk_id
    }

    fn buffer_from_chunk_id(&self, chunk_id: usize) -> &wgpu::Buffer {
        &self.chunks[chunk_id].buffer
    }
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
