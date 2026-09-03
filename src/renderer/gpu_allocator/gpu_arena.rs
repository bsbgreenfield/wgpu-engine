use bytemuck::Pod;

use crate::{
    renderer::{
        GPUAllocationHandle, GPUInstanceHandle, GPUUploadable, InstanceUploadJob, StorageData,
        bind_groups::SharedInstanceData,
        gpu_allocator::{
            AllocMetaData, CHUNK_SIZE, GPUAllocator, GPUChunk, GPUUploadJob, GPUUploadResult,
            UploadIndexJob, UploadMaterialJob, UploadMeshJob, VertexArenaError,
            allocation_table::AllocationTable,
        },
    },
    util::types::{GPUMaterialData, GPUTextureData, ModelVertex, PNUJWVertex, PNUVertex, VIndex},
};

// pub(crate): parameter type of `GPUUploadable::upload`, which is pub(crate).
#[allow(unused)]
pub(crate) struct GPUArena<T: GPUUploadable> {
    max_chunks: usize,
    chunks: Vec<GPUChunk<T>>,
    alloc_table: AllocationTable<T::GPUHandle>,
    label: Option<String>,
    bind_group_layout: Option<wgpu::BindGroupLayout>,
}

impl<T: GPUUploadable> GPUArena<T> {
    pub(in crate::renderer) fn get_first_buffer(&self) -> &wgpu::Buffer {
        &self.chunks[0].buffer
    }

    pub(in crate::renderer) fn add_buffer(&mut self, device: &wgpu::Device) {
        self.chunks.push(T::get_chunk(device));
    }

    pub(in crate::renderer) fn resolve_byte_offset(&self, handle: &T::GPUHandle) -> u32 {
        let meta = self.alloc_table.resolve(handle).unwrap();
        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start
    }

    pub(super) fn get_chunks(&self) -> &[GPUChunk<T>] {
        &self.chunks
    }
    pub(super) fn get_chunks_mut(&mut self) -> &mut [GPUChunk<T>] {
        &mut self.chunks
    }
}

impl<T: SharedInstanceData> GPUArena<T> {
    pub(super) fn register_instance(&mut self, handle: GPUInstanceHandle, slot_idx: usize) {
        self.alloc_table.register_instance(handle, slot_idx);
    }

    pub(super) fn get_meta(
        &mut self,
        slot_idx: usize,
    ) -> Result<&mut AllocMetaData, VertexArenaError> {
        Ok(self
            .alloc_table
            .get_meta(slot_idx)
            .ok_or(VertexArenaError::AllocationSlotNotFound)?)
    }

    pub(super) fn allocate_copy_data(
        &mut self,
        handle: GPUInstanceHandle,
        dst_chunk_id: usize,
        dst_node_id: usize,
    ) {
        self.alloc_table.allocate(handle, dst_chunk_id, dst_node_id);
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
impl<'a> GPUUploadJob for UploadMaterialJob<'a> {
    type GPUHandle = GPUAllocationHandle;
    fn get_data(&self) -> &[u8] {
        self.data
    }
    fn get_handle(&self) -> Self::GPUHandle {
        self.alloc_handle.clone()
    }
}

impl<'a, T: ModelVertex> GPUUploadJob for UploadMeshJob<'a, T> {
    type GPUHandle = GPUAllocationHandle;
    fn get_data(&self) -> &[u8] {
        self.verts
    }
    fn get_handle(&self) -> Self::GPUHandle {
        self.alloc_handle.clone()
    }
}

impl<'a> GPUUploadJob for UploadIndexJob<'a> {
    type GPUHandle = GPUAllocationHandle;
    fn get_data(&self) -> &[u8] {
        self.indices
    }
    fn get_handle(&self) -> Self::GPUHandle {
        self.alloc_handle.clone()
    }
}

impl<T: StorageData> GPUUploadable for T {
    type UploadJob<'a> = InstanceUploadJob<'a, T>;
    type GPUHandle = GPUInstanceHandle;
    const USAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE
        .union(wgpu::BufferUsages::COPY_DST)
        .union(wgpu::BufferUsages::COPY_SRC);
    const LABEL: &'static str = <T as StorageData>::LABEL;
    const MIN_ALLOC_SIZE: u32 = 1024;
    const CHUNK_SIZE: u32 = <T as StorageData>::CHUNK_SIZE;
    fn arena_label() -> String {
        String::from("instance upload")
    }

    fn upload(
        arena: &mut GPUArena<T>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult {
        let slot_idx = arena.alloc_table.allocate(handle, chunk_id, node_id);
        let buffer_element_offset =
            arena.chunks[chunk_id].allocator.resolve(node_id).start / size_of::<T>() as u32;
        return GPUUploadResult::BindGroupUploadResult {
            buffer_element_offset,
            alloc_meta_idx: slot_idx,
        };
    }
}

impl GPUUploadable for VIndex {
    type UploadJob<'a> = UploadIndexJob<'a>;
    type GPUHandle = GPUAllocationHandle;
    const CHUNK_SIZE: u32 = CHUNK_SIZE;
    const MIN_ALLOC_SIZE: u32 = 1024;
    const LABEL: &'static str = "Vertex indices";
    const USAGE: wgpu::BufferUsages = wgpu::BufferUsages::INDEX.union(wgpu::BufferUsages::COPY_DST);
    fn arena_label() -> String {
        String::from("Index Arena")
    }

    fn upload(
        arena: &mut GPUArena<Self>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult {
        arena.alloc_table.allocate(handle, chunk_id, node_id);
        return GPUUploadResult::VertexDataUploadSuccess;
    }
}

impl GPUUploadable for PNUJWVertex {
    type GPUHandle = GPUAllocationHandle;
    type UploadJob<'a> = UploadMeshJob<'a, PNUJWVertex>;
    const CHUNK_SIZE: u32 = CHUNK_SIZE;
    const MIN_ALLOC_SIZE: u32 = 2048;
    const LABEL: &'static str = "PNUJW";
    const USAGE: wgpu::BufferUsages =
        wgpu::BufferUsages::VERTEX.union(wgpu::BufferUsages::COPY_DST);
    fn arena_label() -> String {
        String::from("PNUJW Arena")
    }
    fn upload(
        arena: &mut GPUArena<Self>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult {
        arena.alloc_table.allocate(handle, chunk_id, node_id);
        return GPUUploadResult::VertexDataUploadSuccess;
    }
}
impl GPUUploadable for PNUVertex {
    type GPUHandle = GPUAllocationHandle;
    const MIN_ALLOC_SIZE: u32 = 2048;
    type UploadJob<'a> = UploadMeshJob<'a, PNUVertex>;
    const CHUNK_SIZE: u32 = CHUNK_SIZE;
    const LABEL: &'static str = "PNU";
    const USAGE: wgpu::BufferUsages =
        wgpu::BufferUsages::VERTEX.union(wgpu::BufferUsages::COPY_DST);
    fn arena_label() -> String {
        String::from("PNU Arena")
    }
    fn upload(
        arena: &mut GPUArena<Self>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult {
        arena.alloc_table.allocate(handle, chunk_id, node_id);
        return GPUUploadResult::VertexDataUploadSuccess;
    }
}
impl GPUUploadable for GPUMaterialData {
    type GPUHandle = GPUAllocationHandle;

    type UploadJob<'a> = UploadMaterialJob<'a>;

    const LABEL: &'static str = "Material Data";

    const USAGE: wgpu::BufferUsages =
        wgpu::BufferUsages::STORAGE.union(wgpu::BufferUsages::COPY_DST);

    const CHUNK_SIZE: u32 = CHUNK_SIZE / 4;

    const MIN_ALLOC_SIZE: u32 = 0;

    fn arena_label() -> String {
        String::from("Material Data Arena")
    }

    fn upload(
        arena: &mut GPUArena<Self>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult {
        arena.alloc_table.allocate(handle, chunk_id, node_id);
        return GPUUploadResult::MaterialUploadSucess;
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
                    return Ok(T::upload(self, job.get_handle(), chunk_id, node_id));
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
        let meta = self.alloc_table.resolve(&handle).unwrap();
        let mut range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start = range.start / T::SIZE as u32;
        range.end = range.end / T::SIZE as u32;
        (range, &self.chunks[meta.chunk_id].buffer)
    }

    fn remove(
        &mut self,
        handle: &<T as GPUUploadable>::GPUHandle,
    ) -> Result<(), Self::AllocationError> {
        match self.alloc_table.remove(handle)? {
            Some(meta) => {
                self.chunks[meta.chunk_id].allocator.dealloc(meta.node_id)?;
            }
            None => todo!(),
        }
        Ok(())
    }

    fn dealloc(
        &mut self,
        handle: &<T as GPUUploadable>::GPUHandle,
    ) -> Result<(), Self::AllocationError> {
        self.alloc_table.dealloc(handle)
    }
}
