use std::fmt::Debug;

use crate::{
    common::instance::{GPUInstanceHandle, InstanceHandle},
    renderer::{
        bind_groups::SharedInstanceData,
        gpu_allocator::{AllocMetaData, GPUAllocator, VertexArenaError, gpu_arena::GPUArena},
    },
    util::types::{InverseBindMatrix, JointTransform, LocalTransform},
};

pub trait SharedInstanceArena<T: SharedInstanceData + Debug + bytemuck::Pod> {
    fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError>;
    fn register_copy_binding(
        &mut self,
        slot_idx: usize,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError>;
}

impl<T: SharedInstanceData + bytemuck::Pod + Debug> SharedInstanceArena<T> for GPUArena<T> {
    fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        self.register_instance(*new_handle, slot_index);
        Ok(self.resolve(new_handle).0.start)
    }

    fn register_copy_binding(
        &mut self,
        slot_idx: usize,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        // get the chunk and node id of the slot stored by the prototype
        let AllocMetaData {
            chunk_id, node_id, ..
        } = self.get_meta(slot_idx)?.clone();

        // get the data from the prototype allocation
        let src_range = self.get_chunks()[chunk_id].allocator.resolve(node_id);
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
            dst_location.ok_or(VertexArenaError::MaxAllocationReached)?;

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

        let src_buf = &self.get_chunks()[chunk_id].buffer;
        encoder.copy_buffer_to_buffer(src_buf, src_range.start as u64, &staging, 0, size);

        let dst_buf = &self.get_chunks()[dst_chunk_id].buffer;
        encoder.copy_buffer_to_buffer(&staging, 0, dst_buf, dst_offset, size);

        queue.submit(Some(encoder.finish()));

        // register the new data by inserting alloc metadata for the new instance
        self.allocate_copy_data(*new_handle, dst_chunk_id, dst_node_id);
        // self.alloc_table.register_instance(*new_handle, slot_idx);

        Ok(self.get_chunks()[dst_chunk_id]
            .allocator
            .resolve(dst_node_id)
            .start
            / size_of::<T>() as u32)
    }
}

impl SharedInstanceData for LocalTransform {}
impl SharedInstanceData for JointTransform {}
impl SharedInstanceData for InverseBindMatrix {}

//impl<T: SharedInstanceData + Debug + bytemuck::Pod> InstanceArena<T> {
//    pub fn register_shared_binding(
//        &mut self,
//        slot_index: usize,
//        new_handle: &GPUInstanceHandle,
//    ) -> Result<u32, VertexArenaError> {
//        self.alloc_table.register_instance(*new_handle, slot_index);
//        Ok(self.resolve(new_handle))
//    }
//
//    pub fn register_copy_binding(
//        &mut self,
//        slot_idx: usize,
//        new_handle: &GPUInstanceHandle,
//        queue: &wgpu::Queue,
//        device: &wgpu::Device,
//    ) -> Result<u32, VertexArenaError> {
//        self.copy_binding_impl(slot_idx, new_handle, device, queue)
//    }
//
//    pub fn dealloc(&mut self, handle: &GPUInstanceHandle) -> Result<(), VertexArenaError> {
//        self.alloc_table.dealloc(handle)
//    }
//}
//#[allow(unused)]
//pub struct InstanceArena<T: bytemuck::Pod + Debug> {
//    max_chunks: usize,
//    chunks: Vec<GPUChunk<T>>,
//    alloc_table: AllocationTable,
//    label: Option<String>,
//}
//
//impl<T: StorageData> InstanceArena<T> {
//    pub fn get_first_buffer(&self) -> &wgpu::Buffer {
//        &self.chunks[0].buffer
//    }
//
//    pub fn add_buffer(&mut self, device: &wgpu::Device) {
//        self.chunks.push(T::get_chunk(device));
//    }
//    #[allow(unused)]
//    pub fn buffer_len(&self) -> usize {
//        self.chunks.len()
//    }
//
//    fn copy_binding_impl(
//        &mut self,
//        slot_idx: usize,
//        new_handle: &GPUInstanceHandle,
//        device: &wgpu::Device,
//        queue: &wgpu::Queue,
//    ) -> Result<u32, VertexArenaError> {
//        // get the chunk and node id of the slot stored by the prototype
//        let AllocMetaData {
//            chunk_id, node_id, ..
//        } = self
//            .alloc_table
//            .get_meta(slot_idx)
//            .ok_or(VertexArenaError::AllocationSlotNotFound)?
//            .clone();
//
//        // get the data from the prototype allocation
//        let src_range = self.chunks[chunk_id].allocator.resolve(node_id);
//        let size = (src_range.end - src_range.start) as u64;
//
//        // allocate for new node of size "size"
//        let mut dst_location = None;
//        for (chunk_id, chunk) in self.chunks.iter_mut().enumerate() {
//            if let Ok(node_id) = chunk.allocator.alloc_first(size as u32) {
//                dst_location = Some((chunk_id, node_id));
//                break;
//            }
//        }
//        let (dst_chunk_id, dst_node_id) =
//            dst_location.ok_or(VertexArenaError::MaxAllocationReached)?;
//
//        let dst_offset = self.chunks[dst_chunk_id].allocator.offset_of(dst_node_id);
//
//        // do copying
//        let staging = device.create_buffer(&wgpu::BufferDescriptor {
//            label: Some("copy binding staging buffer"),
//            size,
//            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
//            mapped_at_creation: false,
//        });
//        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//            label: Some("copy bindings"),
//        });
//
//        let src_buf = &self.chunks[chunk_id].buffer;
//        encoder.copy_buffer_to_buffer(src_buf, src_range.start as u64, &staging, 0, size);
//
//        let dst_buf = &self.chunks[dst_chunk_id].buffer;
//        encoder.copy_buffer_to_buffer(&staging, 0, dst_buf, dst_offset, size);
//
//        queue.submit(Some(encoder.finish()));
//
//        // register the new data by inserting alloc metadata for the new instance
//        self.alloc_table
//            .allocate(*new_handle, dst_chunk_id, dst_node_id);
//        // self.alloc_table.register_instance(*new_handle, slot_idx);
//
//        Ok(self.chunks[dst_chunk_id]
//            .allocator
//            .resolve(dst_node_id)
//            .start
//            / size_of::<T>() as u32)
//    }
//
//    pub fn upload<'a>(
//        &mut self,
//        job: InstanceUploadJob<'a, T>,
//        queue: &wgpu::Queue,
//        device: &wgpu::Device,
//    ) -> Result<BindGroupUploadResult, VertexArenaError> {
//        if self.chunks.is_empty() {
//            self.add_buffer(device);
//        }
//        'outer: for (chunk_id, chunk) in self.chunks.iter_mut().enumerate() {
//            match chunk.gpu_alloc(job.data, queue, self.label.as_ref().unwrap()) {
//                Ok((node_id, _)) => {
//                    let slot_idx =
//                        self.alloc_table
//                            .allocate(job.gpu_instance_handle, chunk_id, node_id);
//                    let buffer_offset = self.chunks[chunk_id].allocator.resolve(node_id).start
//                        / size_of::<T>() as u32;
//                    return Ok(BindGroupUploadResult {
//                        buffer_offset,
//                        alloc_meta_idx: slot_idx,
//                    });
//                }
//
//                Err(e) => match e {
//                    VertexArenaError::DataTooLarge(_, _) => {
//                        return Err(e);
//                    }
//                    _ => continue 'outer,
//                },
//            }
//        }
//        Err(VertexArenaError::MaxAllocationReached)
//    }
//
//    pub fn remove(&mut self, handle: &GPUInstanceHandle) -> Result<(), VertexArenaError> {
//        // TODO: if this is called on shared instance data, then we
//        // need to remove ALL meta and then dealloc
//        match self.alloc_table.remove(handle)? {
//            Some(meta) => {
//                self.chunks[meta.chunk_id].allocator.dealloc(meta.node_id)?;
//            }
//            None => todo!(),
//        }
//
//        Ok(())
//    }
//
//    pub fn resolve_byte_offset(&self, handle: &GPUInstanceHandle) -> u32 {
//        let meta = self.alloc_table.resolve(handle).unwrap();
//        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
//        range.start
//    }
//    pub fn resolve(&self, handle: &GPUInstanceHandle) -> u32 {
//        let meta = self.alloc_table.resolve(handle).unwrap();
//        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
//        range.start / size_of::<T>() as u32
//    }
//
//    #[allow(unused)]
//    pub fn resolve_buffer(&self, _instance_handle: &InstanceHandle) -> &wgpu::Buffer {
//        //TODO: if we add more chunks, then this will have to actually resolve
//        &self.chunks[0].buffer
//    }
//
//    pub fn new() -> Self {
//        Self {
//            max_chunks: 1,
//            chunks: vec![],
//            alloc_table: AllocationTable::new(),
//            label: Some("Local Transform arena".to_string()),
//        }
//    }
//}
