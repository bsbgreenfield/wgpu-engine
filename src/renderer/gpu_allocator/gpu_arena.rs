use std::{fmt::Debug, marker::PhantomData};

use bytemuck::Pod;

use crate::{
    common::instance::GPUInstanceHandle,
    renderer::{
        GPUAllocationHandle, GPUUploadable, InstanceUploadJob, StorageData,
        gpu_allocator::{
            CHUNK_SIZE, FreeListAllocator, GPUAllocator, GPUChunk, GPUUploadJob, GPUUploadResult,
            MIMIMUM_INDEX_ALLOCATION_SIZE, MIMIMUM_VERTEX_ALLOCATION_SIZE, UploadIndexJob,
            UploadMeshJob, VertexArenaError, allocation_table::AllocationTable,
        },
    },
    util::types::{LocalTransform, ModelVertex, PNUJWVertex, PNUVertex, VIndex},
};

#[allow(unused)]
pub struct GPUArena<T: GPUUploadable> {
    max_chunks: usize,
    chunks: Vec<GPUChunk<T>>,
    alloc_table: AllocationTable<T::GPUHandle>,
    label: Option<String>,
    bind_group_layout: Option<wgpu::BindGroupLayout>,
}

impl<T: GPUUploadable> GPUArena<T> {
    pub fn get_first_buffer(&self) -> &wgpu::Buffer {
        &self.chunks[0].buffer
    }

    pub fn add_buffer(&mut self, device: &wgpu::Device) {
        self.chunks.push(T::get_chunk(device));
    }

    pub fn resolve_byte_offset(&self, handle: &T::GPUHandle) -> u32 {
        let meta = self.alloc_table.resolve(handle).unwrap();
        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start
    }
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

impl<'a, T: Pod> GPUUploadJob for InstanceUploadJob<'a, T> {
    type GPUHandle = GPUInstanceHandle;
    fn get_data(&self) -> &[u8] {
        self.data
    }
    fn get_handle(&self) -> GPUInstanceHandle {
        self.gpu_instance_handle
    }
}

impl<'a, T: ModelVertex> GPUUploadJob for UploadMeshJob<'a, T> {
    type GPUHandle = u32;
    fn get_data(&self) -> &[u8] {
        self.verts
    }
    fn get_handle(&self) -> Self::GPUHandle {
        self.global_alloc_id
    }
}

impl<'a> GPUUploadJob for UploadIndexJob<'a> {
    type GPUHandle = u32;
    fn get_data(&self) -> &[u8] {
        self.indices
    }
    fn get_handle(&self) -> Self::GPUHandle {
        self.global_alloc_id
    }
}

impl<T: StorageData> GPUUploadable for T {
    type UploadJob<'a> = InstanceUploadJob<'a, T>;
    type GPUHandle = GPUInstanceHandle;
    fn arena_label() -> String {
        String::from("instance upload")
    }
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        Self::get_chunk(device)
    }
}

impl GPUUploadable for VIndex {
    type UploadJob<'a> = UploadIndexJob<'a>;
    type GPUHandle = u32;
    fn arena_label() -> String {
        String::from("Index Arena")
    }
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        GPUChunk::<Self>::new(device)
    }
}

impl GPUUploadable for PNUJWVertex {
    type GPUHandle = u32;
    type UploadJob<'a> = UploadMeshJob<'a, PNUJWVertex>;
    fn arena_label() -> String {
        String::from("PNUJW Arena")
    }
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        GPUChunk::<Self>::new(device)
    }
}
impl GPUUploadable for PNUVertex {
    type GPUHandle = u32;
    type UploadJob<'a> = UploadMeshJob<'a, PNUVertex>;
    fn arena_label() -> String {
        String::from("PNU Arena")
    }
    fn get_chunk(device: &wgpu::Device) -> GPUChunk<Self> {
        GPUChunk::<Self>::new(device)
    }
}

impl<T: GPUUploadable> GPUAllocator<T> for GPUArena<T> {
    type AllocationError = VertexArenaError;

    fn new() -> Self {
        Self {
            max_chunks: 16,
            chunks: vec![],
            alloc_table: AllocationTable::new(),
            label: Some(T::arena_label()),
            bind_group_layout: None,
        }
    }

    fn upload<'a>(
        &mut self,
        job: T::UploadJob<'a>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, Self::AllocationError> {
        if self.chunks.is_empty() {
            self.chunks.push(T::get_chunk(device));
        }
        'outer: for (chunk_id, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(job.get_data(), queue, self.label.as_ref().unwrap()) {
                Ok((node_id, _)) => {
                    let slot_idx = self
                        .alloc_table
                        .allocate(job.get_handle(), chunk_id, node_id);
                    let buffer_offset = self.chunks[chunk_id].allocator.resolve(node_id).start
                        / size_of::<T>() as u32;
                    return Ok(GPUUploadResult::BindGroupUploadResult {
                        buffer_offset,
                        alloc_meta_idx: slot_idx,
                    });
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

    fn resolve(&self, handle: &T::GPUHandle) -> (std::range::Range<u32>, &wgpu::Buffer) {
        // let meta = self.alloc_table.get(&handle.global_allocation_id).unwrap();
        // let mut range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        // range.start = range.start / size_of::<V>() as u32;
        // range.end = range.end / size_of::<V>() as u32;
        // (range, &self.chunks[meta.chunk_id].buffer)
        todo!()
    }

    fn remove(
        &mut self,
        handle: &<T as GPUUploadable>::GPUHandle,
    ) -> Result<(), Self::AllocationError> {
        todo!()
    }

    fn dealloc(
        &mut self,
        handle: &<T as GPUUploadable>::GPUHandle,
    ) -> Result<(), Self::AllocationError> {
        self.alloc_table.dealloc(handle)
    }
}
