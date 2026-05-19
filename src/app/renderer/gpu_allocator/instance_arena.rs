use std::{collections::HashMap, fmt::Debug};

use crate::{
    app::renderer::{
        InstanceUploadJob, StorageData,
        gpu_allocator::{AllocMetaData, GPUInstanceAllocator, InstanceChunk, VertexArenaError},
    },
    util::types::LocalTransform,
    world::instance_manager::InstanceHandle,
};

#[allow(unused)]
pub struct InstanceArena<T: bytemuck::Pod + Debug> {
    max_chunks: usize,
    chunks: Vec<InstanceChunk<T>>,
    alloc_table: HashMap<InstanceHandle, AllocMetaData>,
    label: Option<String>,
}

impl InstanceArena<LocalTransform> {
    pub fn get_first_buffer(&self) -> &wgpu::Buffer {
        &self.chunks[0].buffer
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
                        job.instance_handle.clone(),
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
    fn register_shared_binding(
        &mut self,
        donor: &InstanceHandle,
        new_handle: &InstanceHandle,
    ) -> Result<u32, Self::AllocationError> {
        let meta = self
            .alloc_table
            .get(donor)
            .ok_or(VertexArenaError::HandleNotFound(donor.clone()))?;
        self.alloc_table.insert(
            new_handle.clone(),
            AllocMetaData {
                chunk_id: meta.chunk_id,
                node_id: meta.node_id,
            },
        );
        Ok(self.resolve(new_handle))
    }

    fn resolve(&self, handle: &crate::world::instance_manager::InstanceHandle) -> u32 {
        let meta = self.alloc_table.get(&handle).unwrap();
        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start / size_of::<T>() as u32
    }

    fn resolve_buffer(&self, _instance_handle: &InstanceHandle) -> &wgpu::Buffer {
        //TODO: if we add more chunks, then this will have to actually resolve
        &self.chunks[0].buffer
    }

    #[inline]
    fn bind_group(&self) -> &wgpu::BindGroup {
        &self.chunks[0].bind_group
    }

    fn new(device: &wgpu::Device) -> Self {
        let bgl = T::get_bind_group_layout(device);
        Self {
            max_chunks: 1,
            chunks: vec![T::get_chunk(device, &bgl)],
            alloc_table: HashMap::new(),
            label: Some("Local Transform arena".to_string()),
        }
    }
}
