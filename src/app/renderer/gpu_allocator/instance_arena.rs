use std::{collections::HashMap, fmt::Debug, marker::PhantomData, num::NonZero};

use wgpu::ShaderStages;

use crate::{
    app::renderer::{
        InstanceUploadJob,
        gpu_allocator::{
            AllocMetaData, CHUNK_SIZE, GPUInstanceAllocator, InstanceChunk, VertexArenaError,
            free_list::FreeListAllocator,
        },
    },
    util::types::LocalTransform,
    world::{components::RigidAnimationMode, instance_manager::InstanceHandle},
};

#[allow(unused)]
pub struct InstanceArena<T: bytemuck::Pod + Debug> {
    max_chunks: usize,
    chunks: Vec<InstanceChunk<T>>,
    alloc_table: HashMap<InstanceHandle, AllocMetaData>,
    label: Option<String>,
}

impl InstanceChunk<LocalTransform> {
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
            bind_group: new_bg,
            buffer: buf,
            allocator: FreeListAllocator::new(size_of::<LocalTransform>()),
            _t: PhantomData,
        }
    }
}

impl GPUInstanceAllocator<LocalTransform> for InstanceArena<LocalTransform> {
    type AllocationError = VertexArenaError;

    fn upload<'a>(
        &mut self,
        job: InstanceUploadJob<'a, LocalTransform>,
        queue: &wgpu::Queue,
    ) -> Result<u32, Self::AllocationError> {
        'outer: for (chunk_id, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(&job.data, queue, self.label.as_ref().unwrap()) {
                Ok((node_id, _)) => {
                    self.alloc_table.insert(
                        job.instance_handle.clone(),
                        AllocMetaData::new(chunk_id, node_id),
                    );
                    return Ok(self.chunks[chunk_id].allocator.resolve(node_id).start
                        / size_of::<LocalTransform>() as u32);
                }

                Err(e) => match e {
                    VertexArenaError::DataTooLarge(_, _) => {
                        return Err(e);
                    }
                    _ => continue 'outer,
                },
            }
        }
        // if let Some(alloc_entry) = self.alloc_table.get(job.instance_handle)
        //     && job.data.mode == RigidAnimationMode::Shared
        // {
        //     if let Some(alloc) = self.alloc_table.insert(
        //         job.instance_handle.clone(),
        //         AllocMetaData {
        //             node_id: alloc_entry.node_id,
        //             chunk_id: alloc_entry.chunk_id,
        //         },
        //     ) {
        //         return Ok(self.chunks[alloc.chunk_id]
        //             .allocator
        //             .resolve(alloc.node_id)
        //             .start
        //             / size_of::<LocalTransform>() as u32);
        //     } else {
        //         panic!("there was a fatal error with inserting into the lt alloc table")
        //     }
        // } else {
        // }
        Err(VertexArenaError::MaxAllocationReached)
    }

    fn resolve(&self, handle: &crate::world::instance_manager::InstanceHandle) -> u32 {
        let meta = self.alloc_table.get(&handle).unwrap();
        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        range.start / size_of::<LocalTransform>() as u32
    }

    #[inline]
    fn bind_group(&self) -> &wgpu::BindGroup {
        &self.chunks[0].bind_group
    }

    fn new(device: &wgpu::Device) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("local transform bind group LAYOUT"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        NonZero::new(size_of::<LocalTransform>() as u64).unwrap(),
                    ),
                },
                count: None,
            }],
        });
        Self {
            max_chunks: 1,
            chunks: vec![InstanceChunk::new(device, &bgl)],
            alloc_table: HashMap::new(),
            label: Some("Local Transform arena".to_string()),
        }
    }
}
