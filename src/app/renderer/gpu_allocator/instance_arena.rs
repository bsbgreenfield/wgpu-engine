use std::{collections::HashMap, fmt::Debug};

use crate::{
    app::renderer::{
        InstanceUploadJob, StorageData,
        gpu_allocator::{
            AllocMetaData, GPUChunk, GPUInstanceAllocator, SharedInstanceData, VertexArenaError,
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
    alloc_table: HashMap<GPUInstanceHandle, AllocMetaData>,
    label: Option<String>,
}

impl<T: bytemuck::Pod + Debug> InstanceArena<T> {
    pub fn get_first_buffer(&self) -> &wgpu::Buffer {
        &self.chunks[0].buffer
    }

    fn copy_binding_impl(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<u32, VertexArenaError> {
        let meta = self
            .alloc_table
            .get(donor_handle)
            .ok_or(VertexArenaError::HandleNotFound {
                shared: *new_handle,
                donor: *donor_handle,
            })?;

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

        self.alloc_table.insert(
            *new_handle,
            AllocMetaData {
                chunk_id: dst_chunk_id,
                node_id: dst_node_id,
            },
        );

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
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        self.copy_binding_impl(donor_handle, new_handle, device, queue)
    }
    fn register_shared_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        println!("REGSITERING LOCAL");
        let meta = self
            .alloc_table
            .get(donor_handle)
            .ok_or(VertexArenaError::HandleNotFound {
                shared: new_handle.clone(),
                donor: donor_handle.clone(),
            })?;
        self.alloc_table.insert(
            new_handle.clone(),
            AllocMetaData {
                chunk_id: meta.chunk_id,
                node_id: meta.node_id,
            },
        );
        Ok(self.resolve(new_handle))
    }
}

impl SharedInstanceData for InstanceArena<JointTransform> {
    fn register_copy_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        self.copy_binding_impl(donor_handle, new_handle, device, queue)
    }
    fn register_shared_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        println!("REGISTERING JOINTS");
        let meta = self
            .alloc_table
            .get(donor_handle)
            .ok_or(VertexArenaError::HandleNotFound {
                shared: new_handle.clone(),
                donor: donor_handle.clone(),
            })?;
        self.alloc_table.insert(
            new_handle.clone(),
            AllocMetaData {
                chunk_id: meta.chunk_id,
                node_id: meta.node_id,
            },
        );
        Ok(self.resolve(new_handle))
    }
}

impl SharedInstanceData for InstanceArena<InverseBindMatrix> {
    fn register_copy_binding(
        &mut self,
        _donor_handle: &GPUInstanceHandle,
        _new_handle: &GPUInstanceHandle,
        _queue: &wgpu::Queue,
        _device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError> {
        panic!("should not need to copy ibms")
    }

    fn register_shared_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError> {
        let meta = self
            .alloc_table
            .get(donor_handle)
            .ok_or(VertexArenaError::HandleNotFound {
                shared: new_handle.clone(),
                donor: donor_handle.clone(),
            })?;
        self.alloc_table.insert(
            new_handle.clone(),
            AllocMetaData {
                chunk_id: meta.chunk_id,
                node_id: meta.node_id,
            },
        );
        Ok(self.resolve(new_handle))
    }
}
impl<T: StorageData> GPUInstanceAllocator<T> for InstanceArena<T> {
    type AllocationError = VertexArenaError;
    fn upload<'a>(
        &mut self,
        job: InstanceUploadJob<'a, T>,
        queue: &wgpu::Queue,
    ) -> Result<u32, Self::AllocationError> {
        'outer: for (chunk_id, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(job.data, queue, self.label.as_ref().unwrap()) {
                Ok((node_id, _)) => {
                    self.alloc_table.insert(
                        job.gpu_instance_handle.clone(),
                        AllocMetaData::new(chunk_id, node_id),
                    );
                    return Ok(self.chunks[chunk_id].allocator.resolve(node_id).start
                        / size_of::<T>() as u32);
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

    fn resolve(&self, handle: &GPUInstanceHandle) -> u32 {
        let meta = self.alloc_table.get(&handle).unwrap();
        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start / size_of::<T>() as u32
    }

    fn resolve_buffer(&self, _instance_handle: &InstanceHandle) -> &wgpu::Buffer {
        //TODO: if we add more chunks, then this will have to actually resolve
        &self.chunks[0].buffer
    }

    fn new(device: &wgpu::Device) -> Self {
        Self {
            max_chunks: 1,
            chunks: vec![T::get_chunk(device)],
            alloc_table: HashMap::new(),
            label: Some("Local Transform arena".to_string()),
        }
    }
}
