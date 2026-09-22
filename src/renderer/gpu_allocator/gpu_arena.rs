use crate::{
    renderer::{
        AllocationMask, GPUAllocationHandle, GPUInstanceHandle, InstanceUploadJob, PrototypeHandle,
        gpu_allocator::{
            CHUNK_SIZE, DefaultGPUValue, GPUAllocator, GPUChunk, GPUUploadJob, GPUUploadResult,
            GPUUploadable, TAllocationTable, UploadIndexJob, UploadMaterialJob, UploadMeshJob,
            VertexArenaError,
            allocation_tables::{
                AllocationSlot, AllocationTableError, SharedInstanceData, StorageData,
                asset_alloc_table::{AssetAllocationMeta, SingleAlocationTable},
                shared_instance_alloc_table::{
                    InstanceAllocationTable, SharedInstanceAllocTable, SharedInstanceAllocationSlot,
                },
            },
        },
    },
    util::types::{
        GPUMaterialData, InstanceRecordData, InverseBindMatrix, JointTransform, LocalTransform,
        ModelVertex, PNUJWVertex, PNUVertex, VIndex,
    },
};

// pub(crate): parameter type of `GPUUploadable::upload`, which is pub(crate).
#[allow(unused)]
pub(in crate::renderer) struct GPUArena<T: GPUUploadable> {
    max_chunks: usize,
    chunks: Vec<GPUChunk<T>>,
    alloc_table: T::AllocTable,
    label: Option<String>,
}

//#[allow(private_bounds)]
//#[cfg(test)]
//impl<T: GPUUploadable<GPUHandle = GPUInstanceHandle>> GPUArena<T> {
//    pub(crate) fn get_instance_table(&self) -> &HashMap<GPUInstanceHandle, usize> {
//        self.alloc_table.get_table()
//    }
//    #[cfg(test)]
//    pub fn buffer_offset_of(&self, handle: GPUInstanceHandle) -> u32 {
//        self.resolve_byte_offset(&handle)
//    }
//}
//#[allow(private_bounds)]
//#[cfg(test)]
//impl<T: GPUUploadable<GPUHandle = GPUAllocationHandle>> GPUArena<T> {
//    pub(crate) fn get_alloc_table(&self) -> &HashMap<GPUAllocationHandle, usize> {
//        self.alloc_table.get_table()
//    }
//}

impl<T: GPUUploadable> GPUArena<T> {
    pub(in crate::renderer) fn get_first_buffer(&self) -> &wgpu::Buffer {
        &self.chunks[0].buffer
    }

    pub(in crate::renderer) fn add_buffer(&mut self, device: &wgpu::Device) {
        self.chunks.push(T::get_chunk(device));
    }

    pub(in crate::renderer) fn resolve_element_offset(&self, handle: &T::GPUHandle) -> usize {
        let byte_off = self.resolve_byte_offset(handle) as usize;
        return byte_off / size_of::<T>();
    }

    pub(in crate::renderer) fn resolve_byte_offset(&self, handle: &T::GPUHandle) -> u32 {
        let meta = self
            .alloc_table
            .resolve(handle)
            .expect(format!("cannot resolve for: {}", T::LABEL).as_str());
        let range = self.chunks[meta.chunk()].allocator.resolve(meta.node());
        range.start
    }

    pub(super) fn get_chunks(&self) -> &[GPUChunk<T>] {
        &self.chunks
    }
    pub(super) fn get_chunks_mut(&mut self) -> &mut [GPUChunk<T>] {
        &mut self.chunks
    }
    pub(in crate::renderer) fn ensure_initialized(
        &mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) {
        if self.chunks.is_empty() {
            self.chunks.push(T::get_chunk(device));
            T::init_with_defaults(self, queue, device);
        }
    }
}

#[derive(Debug)]
pub(crate) struct InstanceAllocationResult {
    /// the offset of the allocated data within the chunk
    pub data_offset: u32,
    /// the index of the buffer (chunk) in which this data was placed
    pub chunk_index: u32,
}
impl<T: SharedInstanceData> GPUArena<T> {
    pub fn remove_prototype_binding(
        &mut self,
        prototype: &PrototypeHandle,
    ) -> Result<(), VertexArenaError> {
        println!("realease {}", T::LABEL);

        if let Some(meta) = self.alloc_table.release_prototype(prototype).unwrap() {
            self.chunks[meta.chunk()]
                .allocator
                .dealloc(meta.node())
                .map_err(|_| VertexArenaError::DeallocError)?;
        }

        Ok(())
    }
    pub fn register_shared_binding(
        &mut self,
        new_handle: &GPUInstanceHandle,
    ) -> Result<InstanceAllocationResult, AllocationTableError> {
        self.alloc_table
            .allocate(new_handle.clone(), SharedInstanceAllocationSlot::Shared);
        let (chunk_index, _) = self.alloc_table.get_prototype_meta(new_handle);
        let data_offset = self.resolve(new_handle).0.start;
        Ok(InstanceAllocationResult {
            data_offset,
            chunk_index: chunk_index as u32,
        })
    }

    pub fn register_copy_binding(
        &mut self,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<InstanceAllocationResult, AllocationTableError> {
        let (src_chunk_idx, src_node_idx) = self.alloc_table.get_prototype_meta(new_handle);

        // get the data from the prototype allocation
        let src_range = self.get_chunks()[src_chunk_idx]
            .allocator
            .resolve(src_node_idx);
        let size = (src_range.end - src_range.start) as u64;

        // allocate for new node of size "size"
        let mut dst_location = None;
        for (chunk_id, chunk) in self.get_chunks_mut().iter_mut().enumerate() {
            if let Ok(node_id) = chunk.allocator.alloc_first(size as u32) {
                dst_location = Some((chunk_id, node_id));
                break;
            }
        }
        let (dst_chunk_id, dst_node_id) =
            dst_location.ok_or(AllocationTableError::MaxAllocationReached)?;

        let dst_offset = self.get_chunks()[dst_chunk_id]
            .allocator
            .offset_of(dst_node_id);

        // do copying
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("copy binding staging buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("copy bindings"),
        });

        let src_buf = &self.get_chunks()[src_chunk_idx].buffer;
        encoder.copy_buffer_to_buffer(src_buf, src_range.start as u64, &staging, 0, size);

        let dst_buf = &self.get_chunks()[dst_chunk_id].buffer;
        encoder.copy_buffer_to_buffer(&staging, 0, dst_buf, dst_offset, size);

        queue.submit(Some(encoder.finish()));

        self.alloc_table.allocate(
            *new_handle,
            SharedInstanceAllocationSlot::Copied {
                slot: AssetAllocationMeta::new(dst_chunk_id, dst_node_id),
            },
        );

        let data_offset = self.get_chunks()[dst_chunk_id]
            .allocator
            .resolve(dst_node_id)
            .start
            / size_of::<T>() as u32;
        let chunk_index = dst_chunk_id as u32;

        Ok(InstanceAllocationResult {
            data_offset,
            chunk_index,
        })
    }
}

impl<'a, T: bytemuck::Pod> GPUUploadJob for InstanceUploadJob<'a, T> {
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

impl GPUUploadable for LocalTransform {
    type GPUHandle = GPUInstanceHandle;

    type AllocTable = SharedInstanceAllocTable;

    type UploadJob<'a> = InstanceUploadJob<'a, LocalTransform>;

    const LABEL: &'static str = "Local Transform Upload";

    const USAGE: wgpu::BufferUsages = <LocalTransform as StorageData>::BUFFER_USAGES;

    const CHUNK_SIZE: u32 = 1024 * 16;

    const MIN_ALLOC_SIZE: u32 = 64;

    fn arena_label() -> String {
        String::from("Local transform upload arena")
    }

    fn upload(
        arena: &mut GPUArena<Self>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult {
        let meta = SharedInstanceAllocationSlot::Prototype {
            slot: AssetAllocationMeta::new(chunk_id, node_id),
        };
        arena.alloc_table.allocate(handle, meta);
        GPUUploadResult::PrototypeUploaded
    }
}
impl GPUUploadable for JointTransform {
    type GPUHandle = GPUInstanceHandle;

    type AllocTable = SharedInstanceAllocTable;

    type UploadJob<'a> = InstanceUploadJob<'a, JointTransform>;

    const LABEL: &'static str = "Local Transform Upload";

    const USAGE: wgpu::BufferUsages = <JointTransform as StorageData>::BUFFER_USAGES;
    const MIN_ALLOC_SIZE: u32 = 64;
    const CHUNK_SIZE: u32 = 1024 * 16;

    fn arena_label() -> String {
        String::from("Joint transforms arena")
    }

    fn upload(
        arena: &mut GPUArena<Self>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult {
        let meta = SharedInstanceAllocationSlot::Prototype {
            slot: AssetAllocationMeta::new(chunk_id, node_id),
        };
        arena.alloc_table.allocate(handle, meta);
        GPUUploadResult::PrototypeUploaded
    }
}
impl GPUUploadable for InverseBindMatrix {
    type GPUHandle = GPUInstanceHandle;

    type AllocTable = SharedInstanceAllocTable;

    type UploadJob<'a> = InstanceUploadJob<'a, Self>;

    const LABEL: &'static str = "Local Transform Upload";

    const USAGE: wgpu::BufferUsages = <Self as StorageData>::BUFFER_USAGES;
    const MIN_ALLOC_SIZE: u32 = 8;
    const CHUNK_SIZE: u32 = 1024 * 2;

    fn arena_label() -> String {
        String::from("ibm arena")
    }

    fn upload(
        arena: &mut GPUArena<Self>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult {
        let meta = SharedInstanceAllocationSlot::Prototype {
            slot: AssetAllocationMeta::new(chunk_id, node_id),
        };
        arena.alloc_table.allocate(handle, meta);

        GPUUploadResult::PrototypeUploaded
    }
}

impl GPUUploadable for InstanceRecordData {
    type GPUHandle = GPUInstanceHandle;

    type AllocTable = SingleAlocationTable<GPUInstanceHandle>;

    type UploadJob<'a> = InstanceUploadJob<'a, InstanceRecordData>;

    const LABEL: &'static str = "Global TRansform upload";

    const USAGE: wgpu::BufferUsages = <Self as StorageData>::BUFFER_USAGES;

    const CHUNK_SIZE: u32 = 1024;

    const MIN_ALLOC_SIZE: u32 = 64;

    fn arena_label() -> String {
        String::from("instance record alloc arena")
    }

    fn upload(
        arena: &mut GPUArena<Self>,
        handle: Self::GPUHandle,
        chunk_id: usize,
        node_id: usize,
    ) -> GPUUploadResult {
        arena
            .alloc_table
            .allocate(handle, AssetAllocationMeta::new(chunk_id, node_id));
        let element_offset = arena.resolve_element_offset(&handle);
        return GPUUploadResult::BindGroupUploadResult {
            buffer_element_offset: element_offset as u32,
            chunk_idx: chunk_id as u32,
        };
    }
}

impl GPUUploadable for VIndex {
    type UploadJob<'a> = UploadIndexJob<'a>;
    type GPUHandle = GPUAllocationHandle;
    type AllocTable = SingleAlocationTable<GPUAllocationHandle>;
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
        arena
            .alloc_table
            .allocate(handle, AssetAllocationMeta::new(chunk_id, node_id));
        return GPUUploadResult::VertexDataUploadSuccess;
    }
}

impl GPUUploadable for PNUJWVertex {
    type GPUHandle = GPUAllocationHandle;
    type UploadJob<'a> = UploadMeshJob<'a, PNUJWVertex>;
    type AllocTable = SingleAlocationTable<GPUAllocationHandle>;
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
        arena
            .alloc_table
            .allocate(handle, AssetAllocationMeta::new(chunk_id, node_id));
        return GPUUploadResult::VertexDataUploadSuccess;
    }
}
impl GPUUploadable for PNUVertex {
    type GPUHandle = GPUAllocationHandle;
    const MIN_ALLOC_SIZE: u32 = 2048;
    type UploadJob<'a> = UploadMeshJob<'a, PNUVertex>;
    type AllocTable = SingleAlocationTable<GPUAllocationHandle>;
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
        arena
            .alloc_table
            .allocate(handle, AssetAllocationMeta::new(chunk_id, node_id));
        return GPUUploadResult::VertexDataUploadSuccess;
    }
}
impl DefaultGPUValue for GPUMaterialData {
    fn insert_default(gpu_arena: &mut GPUArena<Self>, queue: &wgpu::Queue, device: &wgpu::Device) {
        let default_data = &[GPUMaterialData {
            base_color_factors: [1., 0.3, 0.2, 1.],
            roughness: 1.,
            metallic: 1.,
            tex_mod: 0,
            _pad: 0,
        }];
        let bytes = bytemuck::cast_slice::<GPUMaterialData, u8>(default_data);
        let default_material_job = UploadMaterialJob {
            data: &bytes,
            alloc_handle: GPUAllocationHandle {
                global_allocation_id: u32::MAX,
                alloc_mask: AllocationMask::all(), // intentionally not a valid mask
            },
        };

        let _ = gpu_arena.upload(default_material_job, queue, device);
    }
}
impl GPUUploadable for GPUMaterialData {
    type GPUHandle = GPUAllocationHandle;

    type UploadJob<'a> = UploadMaterialJob<'a>;
    type AllocTable = SingleAlocationTable<GPUAllocationHandle>;

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
        arena
            .alloc_table
            .allocate(handle, AssetAllocationMeta::new(chunk_id, node_id));
        return GPUUploadResult::MaterialUploadSucess;
    }

    fn init_with_defaults(arena: &mut GPUArena<Self>, queue: &wgpu::Queue, device: &wgpu::Device) {
        <Self as DefaultGPUValue>::insert_default(arena, queue, device);
    }
}
impl<T: GPUUploadable> GPUAllocator<T> for GPUArena<T> {
    type AllocationError = VertexArenaError;

    fn new() -> Self {
        Self {
            max_chunks: 16,
            chunks: vec![],
            alloc_table: T::AllocTable::new(),
            label: Some(T::arena_label()),
        }
    }

    fn upload<'a>(
        &mut self,
        job: T::UploadJob<'a>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<GPUUploadResult, Self::AllocationError> {
        self.ensure_initialized(queue, device);
        'outer: for (chunk_id, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(job.get_data(), queue, self.label.as_ref().unwrap()) {
                Ok((node_id, _)) => {
                    return Ok(T::upload(self, job.get_handle(), chunk_id, node_id));
                }

                Err(e) => match e {
                    VertexArenaError::DataTooLarge(_, _, _) => {
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
        let mut range = self.chunks[meta.chunk()].allocator.resolve(meta.node());
        range.start = range.start / T::SIZE as u32;
        range.end = range.end / T::SIZE as u32;
        (range, &self.chunks[meta.chunk()].buffer)
    }

    fn dealloc(
        &mut self,
        handle: &<T as GPUUploadable>::GPUHandle,
    ) -> Result<(), Self::AllocationError> {
        if let Some(meta) = self
            .alloc_table
            .dealloc(handle)
            .map_err(|_| VertexArenaError::AllocationSlotNotFound)?
        {
            self.chunks[meta.chunk()]
                .allocator
                .dealloc(meta.node())
                .map_err(|_| VertexArenaError::DeallocError)?;
        }
        Ok(())
    }
}
