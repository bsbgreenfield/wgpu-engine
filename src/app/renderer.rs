use std::{any::TypeId, marker::PhantomData};

use crate::{
    util::types::{GlobalTransform, IndexType, InstanceData, ModelVertex, PNUJWVertex},
    world::{
        camera::Camera,
        entity_manager::{EntityHandle, EntityManager},
    },
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
            v: PhantomData::<PNUJWVertex>,
            i: PhantomData::<u16>,
            pipeline,
            views: Vec::new(),
        }
    }
}

pub struct Renderer {
    PNUJW_render_group: Option<RenderGroup<PNUJWVertex, u16>>,
}

impl Renderer {
    pub fn render(&self, device: &wgpu::Device, surface: &wgpu::Surface) {
        Self::render_PNUJW(&self.PNUJW_render_group, device, surface);
    }

    fn render_PNUJW(
        pnujw: &Option<RenderGroup<PNUJWVertex, u16>>,
        device: &wgpu::Device,
        surface: &wgpu::Surface,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let Some(PNUJW_render_group) = pnujw {
            render_pass.set_pipeline(&PNUJW_render_group.pipeline);
            render_pass.set_index_buffer(
                PNUJW_render_group.index_buffer.slice(),
                wgpu::IndexFormat::Uint16,
            );
            for view in PNUJW_render_group.views.iter() {
                for mesh in view.meshes.iter() {}
                render_pass.set_immediates(0, &mesh.id);
                for primitive in mesh.primitives.iter() {
                    render_pass.set_vertex_buffer(0, &PNUJW_render_group.vertex_buffer);
                    render_pass.draw_indexed(primitive.indices.clone(), 0, 0..1);
                }
            }
        }

        Ok(())
    }

    fn setup_for(&mut self, entity: EntityHandle, entity_manager: &EntityManager) {
        todo!()
    }

    fn ensure_render_group<V: ModelVertex, I: IndexType>(
        &mut self,
        device: &wgpu::Device,
        format: &wgpu::TextureFormat,
    ) -> Result<(), RendererError> {
        let v = TypeId::of::<V>();
        let i = TypeId::of::<I>();

        if v == TypeId::of::<PNUJWVertex>() && i == TypeId::of::<u16>() {
            if self.PNUJW_render_group.is_none() {
                self.PNUJW_render_group = Some(RenderGroup::new(device, *format));
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
        if let Some(pnuj) = &self.PNUJW_render_group {
            render_pass.set_pipeline(&pnuj.pipeline);
        }
        Ok(())
    }
}
