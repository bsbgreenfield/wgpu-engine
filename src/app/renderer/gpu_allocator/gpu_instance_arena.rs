use std::{collections::HashMap, fmt::Debug};

use crate::{
    app::renderer::{
        GPUInstanceHandle, InstanceUploadJob, StorageData,
        gpu_allocator::{
            AllocMetaData, GPUChunk, GPUInstanceAllocator, SharedInstanceData, VertexArenaError,
        },
    },
    util::types::{JointTransform, LocalTransform},
    world::instance_manager::InstanceHandle,
};

#[allow(unused)]
pub struct GPUInstanceArena<T: bytemuck::Pod + Debug> {
    max_chunks: usize,
    chunks: Vec<GPUChunk<T>>,
    alloc_table: HashMap<GPUInstanceHandle, AllocMetaData>,
    label: Option<String>,
}

impl<T: bytemuck::Pod + Debug> GPUInstanceArena<T> {
    pub fn get_first_buffer(&self) -> &wgpu::Buffer {
        &self.chunks[0].buffer
    }
}

impl SharedInstanceData for GPUInstanceArena<LocalTransform> {
    fn register_shared_binding(
        &mut self,
        donor_handle: &InstanceHandle,
        new_handle: &InstanceHandle,
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

impl SharedInstanceData for GPUInstanceArena<JointTransform> {
    fn register_shared_binding(
        &mut self,
        donor_handle: &InstanceHandle,
        new_handle: &InstanceHandle,
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

impl<T: StorageData> GPUInstanceAllocator<T> for GPUInstanceArena<T> {
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
                        job.instance_alloc_handle,
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
