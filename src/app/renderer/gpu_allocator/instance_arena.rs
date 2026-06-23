use std::fmt::Debug;

use crate::{
    app::renderer::{
        InstanceUploadJob, StorageData,
        bind_groups::{BindGroupUploadResult, SharedInstanceData},
        gpu_allocator::{
            GPUChunk, GPUInstanceAllocator, VertexArenaError, allocation_table::AllocationTable,
        },
        renderer::GPUInstanceHandle,
    },
    util::types::{InverseBindMatrix, JointTransform, LocalTransform},
    world::instance_manager::InstanceHandle,
};

#[allow(unused)]
pub struct InstanceArena<T: bytemuck::Pod + Debug> {
    max_chunks: usize,
    chunks: Vec<GPUChunk<T>>,
    alloc_table: AllocationTable,
    label: Option<String>,
}

impl<T: StorageData> InstanceArena<T> {
    pub fn get_first_buffer(&self) -> &wgpu::Buffer {
        &self.chunks[0].buffer
    }

    pub fn add_buffer(&mut self, device: &wgpu::Device) {
        self.chunks.push(T::get_chunk(device));
    }
    #[allow(unused)]
    pub fn buffer_len(&self) -> usize {
        self.chunks.len()
    }

    fn copy_binding_impl(
        &mut self,
        slot_idx: usize,
        new_handle: &GPUInstanceHandle,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<u32, VertexArenaError> {
        let meta = self
            .alloc_table
            .get_meta(slot_idx)
            .ok_or(VertexArenaError::AllocationSlotNotFound)?;

        let src_range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        let size = (src_range.end - src_range.start) as u64;

        let mut dst_location = None;
        for (chunk_id, chunk) in self.chunks.iter_mut().enumerate() {
            if let Ok(node_id) = chunk.allocator.alloc_first(size as u32) {
                dst_location = Some((chunk_id, node_id));
                break;
            }
        }
        let (dst_chunk_id, dst_node_id) =
            dst_location.ok_or(VertexArenaError::MaxAllocationReached)?;

        let dst_offset = self.chunks[dst_chunk_id].allocator.offset_of(dst_node_id);

        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("copy binding staging buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("copy bindings"),
        });

        let src_buf = &self.chunks[meta.chunk_id].buffer;
        encoder.copy_buffer_to_buffer(src_buf, src_range.start as u64, &staging, 0, size);

        let dst_buf = &self.chunks[dst_chunk_id].buffer;
        encoder.copy_buffer_to_buffer(&staging, 0, dst_buf, dst_offset, size);

        queue.submit(Some(encoder.finish()));

        self.alloc_table.register_instance(*new_handle, slot_idx);

        Ok(self.chunks[dst_chunk_id]
            .allocator
            .resolve(dst_node_id)
            .start
            / size_of::<T>() as u32)
    }
}

impl SharedInstanceData for InstanceArena<LocalTransform> {
    fn register_copy_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        self.copy_binding_impl(slot_index, new_handle, device, queue)
    }
    fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        self.alloc_table.register_instance(*new_handle, slot_index);
        Ok(self.resolve(new_handle))
    }
}

impl SharedInstanceData for InstanceArena<JointTransform> {
    fn register_copy_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        self.copy_binding_impl(slot_index, new_handle, device, queue)
    }
    fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        self.alloc_table.register_instance(*new_handle, slot_index);
        Ok(self.resolve(new_handle))
    }
}

impl SharedInstanceData for InstanceArena<InverseBindMatrix> {
    fn register_copy_binding(
        &mut self,
        _slot_index: usize,
        _new_handle: &GPUInstanceHandle,
        _queue: &wgpu::Queue,
        _device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        panic!("should not need to copy ibms")
    }

    fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        self.alloc_table.register_instance(*new_handle, slot_index);
        Ok(self.resolve(new_handle))
    }
}

impl<T: StorageData> GPUInstanceAllocator<T> for InstanceArena<T> {
    type AllocationError = VertexArenaError;
    fn upload<'a>(
        &mut self,
        job: InstanceUploadJob<'a, T>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<BindGroupUploadResult, Self::AllocationError> {
        if self.chunks.is_empty() {
            self.add_buffer(device);
        }
        'outer: for (chunk_id, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(job.data, queue, self.label.as_ref().unwrap()) {
                Ok((node_id, _)) => {
                    let slot_idx =
                        self.alloc_table
                            .allocate(job.gpu_instance_handle, chunk_id, node_id);
                    let buffer_offset = self.chunks[chunk_id].allocator.resolve(node_id).start
                        / size_of::<T>() as u32;
                    return Ok(BindGroupUploadResult {
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

    fn remove(&mut self, handle: &GPUInstanceHandle) -> Result<(), Self::AllocationError> {
        match self.alloc_table.dealloc(handle)? {
            Some(meta) => {
                self.chunks[meta.chunk_id].allocator.dealloc(meta.node_id)?;
            }
            None => {
                // TODO: verify that this is correct
            }
        }
        Ok(())
    }

    fn resolve(&self, handle: &GPUInstanceHandle) -> u32 {
        let meta = self.alloc_table.resolve(handle).unwrap();
        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start / size_of::<T>() as u32
    }

    fn resolve_buffer(&self, _instance_handle: &InstanceHandle) -> &wgpu::Buffer {
        //TODO: if we add more chunks, then this will have to actually resolve
        &self.chunks[0].buffer
    }

    fn new() -> Self {
        Self {
            max_chunks: 1,
            chunks: vec![],
            alloc_table: AllocationTable::new(),
            label: Some("Local Transform arena".to_string()),
        }
    }
}
