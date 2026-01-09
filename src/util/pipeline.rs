pub trait InstanceData {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

// TODO: add generic parameter for the pipeline format
pub trait PipelineUniform {
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
    fn create_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer;
    fn create_bind_group(
        buffer: &wgpu::Buffer,
        bgl: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup;
}
