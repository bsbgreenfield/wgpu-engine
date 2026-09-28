use wgpu::RenderPass;

use crate::renderer::RenderCategory;

pub(super) struct EngineRenderPass {
    pub(super) label: String,
    pub(super) categories: Vec<RenderCategory>,
}

impl EngineRenderPass {
    pub(super) fn create_pass<'frame>(
        label: &'frame str,
        encoder: &'frame mut wgpu::CommandEncoder,
        view: &'frame wgpu::TextureView,
        depth_view: &'frame wgpu::TextureView,
    ) -> Result<RenderPass<'frame>, wgpu::CreateSurfaceError> {
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.5,
                        g: 0.7,
                        b: 1.,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        Ok(render_pass)
    }
}
