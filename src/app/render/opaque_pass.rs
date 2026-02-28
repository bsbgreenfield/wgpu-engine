use std::marker::PhantomData;

use crate::{
    app::{
        app_config::AppConfig,
        render::{
            OpaquePass,
            render_group::{RenderGroup, RenderGroupType, RenderView},
        },
    },
    util::types::{GlobalTransform, InstanceData, ModelVertex, PNUJWVertex, PNUVertex},
    world::camera::Camera,
};

impl RenderGroupType for OpaquePass {
    fn create_pass<'pass>() -> wgpu::RenderPass<'pass> {
        todo!()
    }

    fn create_pipelines(
        config: &AppConfig,
        shader: &wgpu::ShaderModule,
    ) -> Vec<wgpu::RenderPipeline> {
        let pipeline_layout =
            config
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[
                        &Camera::get_bind_group_layout(&config.device),
                        //LOCAL TRANSFORMS,
                    ],
                    immediate_size: 4,
                });
        let pipeline = config
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                multiview_mask: None,
                label: Some("Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[PNUJWVertex::desc(), GlobalTransform::desc()], // vertex input,
                    // intanceInput
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.surface_config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::all(),
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None, // Some(wgpu::DepthStencilState {
                //         format: wgpu::TextureFormat::Depth32Float,
                //         depth_write_enabled: true,
                //         depth_compare: wgpu::CompareFunction::Less,
                //         stencil: wgpu::StencilState::default(),
                //         bias: wgpu::DepthBiasState::default(),
                //     })
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                cache: None,
            });
        vec![pipeline]
    }
}
