use std::{any::TypeId, collections::HashMap, fmt::Display, marker::PhantomData, ops::Range};

use crate::{
    app::renderer_new::{
        CHUNK_SIZE,
        free_list::{FreeListAllocError, FreeListAllocator},
    },
    asset_manager::gltf_assets::model_builder_new::GltfMeshData,
    util::types::{LocalTransform, ModelVertex},
};

pub(super) enum VertexArenaError {
    DataTooLarge(u32),
    FreeListError(FreeListAllocError),
}

impl From<FreeListAllocError> for VertexArenaError {
    fn from(value: FreeListAllocError) -> Self {
        Self::FreeListError(value)
    }
}

impl Display for VertexArenaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::DataTooLarge(size) => f.write_str(
                format!(
                    "cannot allocate mesh of size {}, which exceeds chunk size: {}",
                    size, CHUNK_SIZE
                )
                .as_str(),
            ),
            Self::FreeListError(err) => err.fmt(f),
        }
    }
}

struct GPUChunk<T: bytemuck::Pod> {
    remaining_space: u32,
    buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    allocator: FreeListAllocator,
    _t: PhantomData<T>,
}

#[derive(Hash, PartialEq, Eq)]
pub(super) struct AllocationHandle {
    pub(super) global_alloc_id: u32,
    pipeline_alloc_id: u32,
}

impl GPUChunk<LocalTransform> {
    fn new(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout) -> Self {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: CHUNK_SIZE as u64,
            usage: wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let new_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lt bind group"),
            layout: bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 1,
                resource: buf.as_entire_binding(),
            }],
        });
        Self {
            remaining_space: CHUNK_SIZE, // TODO: different sizes for diff types?
            bind_group: Some(new_bg),
            buffer: buf,
            allocator: FreeListAllocator::new(),
            _t: PhantomData,
        }
    }
}

impl<T: bytemuck::Pod> GPUChunk<T> {
    fn gpu_alloc(
        &mut self,
        data: &[T],
        queue: &wgpu::Queue,
    ) -> Result<(usize, Range<u32>), VertexArenaError> {
        let size = (data.len() * size_of::<T>()) as u32;
        let node_idx: usize = if self.remaining_space >= size {
            self.allocator.alloc_first(size)?
        } else {
            return Err(VertexArenaError::DataTooLarge(size));
        };
        let offset = self.allocator.offset_of(node_idx) as u32;
        queue.write_buffer(&self.buffer, offset.into(), bytemuck::cast_slice(data));
        Ok((node_idx, offset..offset + size))
    }
}

impl<T: ModelVertex> GPUChunk<T> {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            remaining_space: CHUNK_SIZE,
            bind_group: None,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: CHUNK_SIZE as u64,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
            allocator: FreeListAllocator::new(),
            _t: PhantomData,
        }
    }
}

struct AllocMetaData {
    chunk_id: usize,
    node_id: usize,
}
impl AllocMetaData {
    fn new(chunk_id: usize, node_id: usize) -> Self {
        Self { chunk_id, node_id }
    }
}

pub(super) struct GPUArenaNew<T: bytemuck::Pod> {
    max_chunks: usize,
    chunks: Vec<GPUChunk<T>>,
    alloc_table: HashMap<u32, AllocMetaData>,
    bind_group_layout: Option<wgpu::BindGroupLayout>,
}

impl GPUArenaNew<LocalTransform> {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(" lt bind group LAYOUT"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                count: None,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                visibility: wgpu::ShaderStages::VERTEX,
            }],
        });
        Self {
            max_chunks: 1,
            chunks: vec![GPUChunk::<LocalTransform>::new(device, &bgl)],
            alloc_table: HashMap::new(),
            bind_group_layout: Some(bgl),
        }
    }

    pub(super) fn upload(
        &mut self,
        local_transforms: &[LocalTransform],
        global_alloc_id: u32,
        queue: &wgpu::Queue,
    ) -> Result<(), VertexArenaError> {
        let (node_id, _range) = self.chunks[0].gpu_alloc(local_transforms, queue)?;
        self.alloc_table.insert(
            global_alloc_id,
            AllocMetaData {
                chunk_id: 0,
                node_id,
            },
        );

        Ok(())
    }

    pub(super) fn get_bind_group(&self) -> &wgpu::BindGroup {
        return self.chunks[0].bind_group.as_ref().unwrap();
    }

    pub(super) fn resolve_lt_index(&self, local_mesh_id: u32, global_alloc_id: u32) -> u32 {
        let node_id = self.alloc_table.get(&global_alloc_id).unwrap().node_id;
        let allocation_range = self.chunks[0].allocator.resolve(node_id);

        // the local mesh id should be "local" to the given allocation.
        // therefore, the "global" mesh id should be the local id + the global offset
        // we can obtain the index offset by dividing by the byte size of LocalTransform
        // which is 128
        (allocation_range.start / size_of::<LocalTransform>() as u32) + local_mesh_id
    }
}

impl<V: ModelVertex> GPUArenaNew<V> {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        Self {
            bind_group_layout: None,
            max_chunks: 16,
            chunks: vec![GPUChunk::<V>::new(device)],
            alloc_table: HashMap::new(),
        }
    }

    pub(super) fn resolve(&self, handle: &AllocationHandle) -> (Range<u32>, &wgpu::Buffer) {
        let meta = self.alloc_table.get(&handle.pipeline_alloc_id).unwrap();
        let range = self.chunks[meta.chunk_id].allocator.resolve(meta.node_id);
        (range, &self.chunks[meta.chunk_id].buffer)
    }

    pub(super) fn upload_mesh(
        &mut self,
        mesh_data: &[V],
        global_alloc_id: u32,
        queue: &wgpu::Queue,
    ) -> Result<AllocationHandle, VertexArenaError> {
        'outer: for (chunk_idx, chunk) in self.chunks.iter_mut().enumerate() {
            match chunk.gpu_alloc(mesh_data, queue) {
                Ok((node_idx, _)) => {
                    let pipeline_alloc_id = self.alloc_table.len() as u32; // TODO: algorithm for assigning keys
                    self.alloc_table
                        .insert(pipeline_alloc_id, AllocMetaData::new(chunk_idx, node_idx));
                    return Ok(AllocationHandle {
                        global_alloc_id,
                        pipeline_alloc_id,
                    });
                }

                Err(e) => match e {
                    VertexArenaError::DataTooLarge(_) => {
                        return Err(e);
                    }
                    _ => continue 'outer,
                },
            }
        }
        Err(VertexArenaError::DataTooLarge(
            (mesh_data.len() * size_of::<V>()) as u32,
        ))
    }
}
