use crate::{
    app::{app_config::AppConfig, renderer::renderer::RenderCategory},
    util::types::{
        GlobalTransform, InstanceData, LocalTransform, ModelVertex, PNUJWVertex, StorageData,
    },
    world::camera::Camera,
};

pub struct EnginePipeline {
    pub(super) pipeline: wgpu::RenderPipeline,
}

impl EnginePipeline {}

pub struct PipelineCollection {
    pub opaque_skinned: EnginePipeline,
    pub opaque_static: EnginePipeline,
}

impl PipelineCollection {
    pub(super) fn new(config: &AppConfig) -> Self {
        use super::renderer::RenderCategory::*;
        Self {
            opaque_skinned: Self::create_pipeline(OpaqueStatic, config),
            opaque_static: Self::create_pipeline(OpaqueSkinned, config),
        }
    }

    fn opaque_static_layout(device: &wgpu::Device) -> wgpu::PipelineLayout {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("opaque_static pipeline layout"),
            bind_group_layouts: &[
                &Camera::get_bind_group_layout(device),
                &LocalTransform::get_bind_group_layout(device),
            ],
            immediate_size: 4,
        })
    }

    fn create_pipeline(cat: RenderCategory, config: &AppConfig) -> EnginePipeline {
        match cat {
            RenderCategory::OpaqueStatic => {
                let shader = config
                    .device
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("opaque_static shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_str!("../../static_shader.wgsl").into(),
                        ),
                    });
                let layout = Self::opaque_static_layout(&config.device);

                let color_targets: [Option<wgpu::ColorTargetState>; 1] =
                    if config.surface_config.as_ref().is_none() {
                        [Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::all(),
                        })]
                    } else {
                        [Some(wgpu::ColorTargetState {
                            format: config.surface_config.as_ref().unwrap().format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::all(),
                        })]
                    };
                let pipeline =
                    config
                        .device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some("opaque_static pipeline"),
                            layout: Some(&layout),
                            vertex: wgpu::VertexState {
                                module: &shader,
                                entry_point: Some("vs_main"),
                                buffers: &[PNUJWVertex::desc(), GlobalTransform::desc()],
                                compilation_options: Default::default(),
                            },
                            primitive: wgpu::PrimitiveState {
                                topology: wgpu::PrimitiveTopology::TriangleList,
                                strip_index_format: None,
                                front_face: wgpu::FrontFace::Ccw,
                                cull_mode: Some(wgpu::Face::Back),
                                unclipped_depth: false,
                                conservative: false,
                                polygon_mode: wgpu::PolygonMode::Fill,
                            },
                            depth_stencil: None,
                            multisample: wgpu::MultisampleState {
                                count: 1,
                                mask: !0,
                                alpha_to_coverage_enabled: false,
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: &shader,
                                entry_point: Some("fs_main"),
                                compilation_options: Default::default(),
                                targets: &color_targets,
                            }),
                            multiview_mask: None,
                            cache: None,
                        });

                EnginePipeline { pipeline }
            }
            RenderCategory::OpaqueSkinned => {
                let shader = config
                    .device
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("opaque_static shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_str!("../../static_shader.wgsl").into(),
                        ),
                    });
                let layout = Self::opaque_static_layout(&config.device);

                let color_targets: [Option<wgpu::ColorTargetState>; 1] =
                    if config.surface_config.as_ref().is_none() {
                        [Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::all(),
                        })]
                    } else {
                        [Some(wgpu::ColorTargetState {
                            format: config.surface_config.as_ref().unwrap().format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::all(),
                        })]
                    };
                let pipeline =
                    config
                        .device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some("opaque_static pipeline"),
                            layout: Some(&layout),
                            vertex: wgpu::VertexState {
                                module: &shader,
                                entry_point: Some("vs_main"),
                                buffers: &[PNUJWVertex::desc(), GlobalTransform::desc()],
                                compilation_options: Default::default(),
                            },
                            primitive: wgpu::PrimitiveState {
                                topology: wgpu::PrimitiveTopology::TriangleList,
                                strip_index_format: None,
                                front_face: wgpu::FrontFace::Ccw,
                                cull_mode: Some(wgpu::Face::Back),
                                unclipped_depth: false,
                                conservative: false,
                                polygon_mode: wgpu::PolygonMode::Fill,
                            },
                            depth_stencil: None,
                            multisample: wgpu::MultisampleState {
                                count: 1,
                                mask: !0,
                                alpha_to_coverage_enabled: false,
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: &shader,
                                entry_point: Some("fs_main"),
                                compilation_options: Default::default(),
                                targets: &color_targets,
                            }),
                            multiview_mask: None,
                            cache: None,
                        });

                EnginePipeline { pipeline }
            }
        }
    }
}
