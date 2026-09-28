use crate::{
    renderer::{
        AllocationMask, GPUAllocationHandle,
        gpu_allocator::{GPUAllocator, UploadMeshJob, VertexArenaError, gpu_arena::GPUArena},
        renderer::Renderer,
    },
    util::types::{ModelVertex, PNUJWVertex, PNUVertex, VIndex16, VIndex32},
};

pub(super) struct VertexArenaCollection {
    pub(super) index_arena_16: GPUArena<VIndex16>,
    pub(super) index_arena_32: GPUArena<VIndex32>,
    pub(super) static_arena: GPUArena<PNUVertex>,
    pub(super) skinned_arena: GPUArena<PNUJWVertex>,
}

impl VertexArenaCollection {
    pub(super) fn new() -> Self {
        Self {
            index_arena_16: GPUArena::<VIndex16>::new(),
            index_arena_32: GPUArena::<VIndex32>::new(),
            static_arena: GPUArena::<PNUVertex>::new(),
            skinned_arena: GPUArena::<PNUJWVertex>::new(),
        }
    }
    pub(super) fn resolve_indices(
        &self,
        handle: &GPUAllocationHandle,
    ) -> Option<(std::range::Range<u32>, &wgpu::Buffer, wgpu::IndexFormat)> {
        if handle.alloc_mask.contains(AllocationMask::INDEX32) {
            let (range, buffer) = self.index_arena_32.resolve(handle);
            Some((range, buffer, wgpu::IndexFormat::Uint32))
        } else if handle.alloc_mask.contains(AllocationMask::INDEX16) {
            let (range, buffer) = self.index_arena_16.resolve(handle);
            Some((range, buffer, wgpu::IndexFormat::Uint16))
        } else {
            None
        }
    }
}
pub(super) trait VertexArenaSelector<V: ModelVertex> {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<V>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError>;
}
impl VertexArenaSelector<PNUJWVertex> for Renderer {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<PNUJWVertex>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError> {
        let _handle = self
            .vertex_arenas
            .skinned_arena
            .upload(mesh_job, queue, device)?;
        Ok(())
    }
}

impl VertexArenaSelector<PNUVertex> for Renderer {
    fn upload_mesh(
        &mut self,
        mesh_job: UploadMeshJob<PNUVertex>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<(), VertexArenaError> {
        let _handle = self
            .vertex_arenas
            .static_arena
            .upload(mesh_job, queue, device)?;
        // TODO handle?
        Ok(())
    }
}
