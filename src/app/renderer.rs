use std::{any::TypeId, marker::PhantomData};

use crate::{
    app::app_config::AppConfig,
    util::types::{GlobalTransform, IndexType, ModelVertex, PNUJWVertex},
    world::camera::Camera,
};

#[derive(Debug)]
enum RendererError {
    UndefinedRenderGroup(TypeId, TypeId),
}

struct RenderView {
    vertex_offset_len: (u64, u64),
    index_offset_len: Option<(u64, u64)>,
}

struct RenderGroup<V: ModelVertex, I: IndexType> {
    v: PhantomData<V>,
    i: PhantomData<I>,
    pipeline: wgpu::RenderPipeline,
    views: Vec<RenderView>,
}

impl RenderGroup<PNUJWVertex, u16> {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[
                &Camera::get_bind_group_layout(device),
                //LOCAL TRANSFORMS,
            ],
            immediate_size: 4,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    format: format,
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

        Self {
            v: PhantomData::<PNUJWVertex>(),
            i: PhantomData::<u16>(),
            pipeline,
            views: Vec::new(),
        }
    }
}

pub struct Renderer {
    PNUJW_render_group: Option<RenderGroup<PNUJWVertex, u16>>,
}

impl Renderer {
    fn ensure_render_group<V: ModelVertex, I: IndexType>(
        &mut self,
        device: &wgpu::Device,
        format: &wgpu::TextureFormat,
    ) -> Result<(), RendererError> {
        let v = TypeId::of::<V>();
        let i = TypeId::of::<I>();

        if v == TypeId::of::<PNUJWVertex>() && i == TypeId::of::<u16>() {
            if self.PNUJW_render_group.is_none() {
                self.PNUJW_render_group =
                    Some(RenderGroup::new::<PNUJWVertex, u16>(device, *format));
            }
            return Ok(());
        } else {
            return Err(RendererError::UndefinedRenderGroup(v, i));
        }
    }

    pub(super) fn new() -> Self {
        Renderer {
            PNUJW_render_group: None,
        }
    }

    pub(super) fn render(
        &self,
        render_pass: &mut wgpu::RenderPass,
    ) -> Result<(), wgpu::SurfaceError> {
        render_pass.set_pipeline(&self.pipeline);
        todo!()
    }
}
