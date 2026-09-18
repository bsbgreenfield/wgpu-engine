use std::fmt::Debug;

use crate::{
    renderer::{
        GPUInstanceHandle,
        bind_groups::SharedInstanceData,
        gpu_allocator::{AllocMetaData, GPUAllocator, VertexArenaError, gpu_arena::GPUArena},
    },
    util::types::{InverseBindMatrix, JointTransform, LocalTransform},
};

#[derive(Debug)]
pub(crate) struct InstanceAllocationResult {
    /// the offset of the allocated data within the chunk
    pub data_offset: u32,
    /// the index of the buffer (chunk) in which this data was placed
    pub chunk_index: u32,
}

pub(in crate::renderer) trait SharedInstanceArena<T: SharedInstanceData + Debug + bytemuck::Pod> {
    fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<InstanceAllocationResult, VertexArenaError>;
    fn register_copy_binding(
        &mut self,
        slot_idx: usize,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<InstanceAllocationResult, VertexArenaError>;
}

impl<T: SharedInstanceData + bytemuck::Pod + Debug> SharedInstanceArena<T> for GPUArena<T> {
    fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<InstanceAllocationResult, VertexArenaError> {
        self.register_instance(*new_handle, slot_index);
        let data_offset = self.resolve(new_handle).0.start;
        let chunk_index = self.get_meta(slot_index)?.chunk_id as u32;
        Ok(InstanceAllocationResult {
            data_offset,
            chunk_index,
        })
    }

    fn register_copy_binding(
        &mut self,
        slot_idx: usize,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<InstanceAllocationResult, VertexArenaError> {
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

impl SharedInstanceData for LocalTransform {}
impl SharedInstanceData for JointTransform {}
impl SharedInstanceData for InverseBindMatrix {}
