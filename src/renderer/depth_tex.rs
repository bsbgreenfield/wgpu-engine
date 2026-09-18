pub(super) struct DepthTexture {
    #[allow(unused)]
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
}

impl DepthTexture {
    pub(super) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub(super) fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // a zero-sized texture is invalid, and headless configs report a size of 0
        let size = wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view }
    }

    pub(super) fn depth_stencil_state() -> wgpu::DepthStencilState {
        wgpu::DepthStencilState {
            format: Self::FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }
    }
}
