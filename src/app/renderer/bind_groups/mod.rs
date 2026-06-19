use crate::{
    app::renderer::{gpu_allocator::VertexArenaError, renderer::GPUInstanceHandle},
    world::instance_manager::InstanceHandle,
};

pub(super) mod instance_data;
pub(super) mod local_transforms;
pub(super) mod skinning;

pub(super) trait BindGroupProvider {
    fn get_bind_group(&self, alloc_handle: &InstanceHandle) -> &wgpu::BindGroup;
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
    fn add_bind_group(&mut self, device: &wgpu::Device);
    fn new() -> Self;
}
pub(super) trait SharedInstanceData {
    fn register_shared_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError>;

    fn register_copy_binding(
        &mut self,
        donor_handle: &GPUInstanceHandle,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError>;
}
