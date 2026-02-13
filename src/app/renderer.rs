use std::{any::TypeId, marker::PhantomData, ops::Range};

use wgpu::BufferSlice;

use crate::{
    app::app_config::AppConfig,
    util::types::{GlobalTransform, IndexType, InstanceData, ModelVertex, PNUJWVertex, PNUVertex},
    world::{
        camera::Camera,
        entity_manager::{EntityHandle, EntityManager},
        world::WorldUpdateDelta,
    },
};

#[derive(Debug)]
enum RendererError {
    UndefinedRenderGroup(TypeId, TypeId),
}

struct UploadMeshJob<'j, V: ModelVertex, I: IndexType> {
    vertices: &'j [V],
    indices: &'j [I],
}

struct ArenaAllocator {
    cursor: u64,
}

impl ArenaAllocator {
    fn alloc(&mut self, size: u64, align: u64) -> Option<Range<u64>> {
        let aligned = Self::align_up(self.cursor, align);
        let end = aligned + size;
        self.cursor = end;
        Some(aligned..end)
    }

    fn align_up(value: u64, align: u64) -> u64 {
        (value + align - 1) & !(align - 1)
    }
}

struct GPUMeshArena<V: ModelVertex, I: IndexType> {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_capacity: u64,
    vertex_allocator: ArenaAllocator,
    index_allocator: ArenaAllocator,
    vertex_type: PhantomData<V>,
    index_type: PhantomData<I>,
}

struct GPUMeshHandle {
    vertex: Range<u64>,
    index: Range<u64>,
    count: u64,
}

impl<V: ModelVertex> GPUMeshArena<V, u16> {
    fn new(device: &wgpu::Device, vertex_capacity: u64, index_capacity: u64) -> Self {
        let label: String = format!("vertex buffer arena for {:?}", TypeId::of::<V>());
        GPUMeshArena {
            vertex_capacity,
            index_capacity,
            vertex_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&label),
                size: vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            index_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("PNUJ vertex buffer arena"),
                size: vertex_capacity,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            vertex_allocator: ArenaAllocator { cursor: 0 },
            index_allocator: ArenaAllocator { cursor: 0 },
            vertex_type: PhantomData::<V>,
            index_type: PhantomData::<u16>,
        }
    }

    fn upload_mesh(
        &mut self,
        upload_job: UploadMeshJob<V, u16>,
        queue: &wgpu::Queue,
    ) -> Option<GPUMeshHandle> {
        let vertex_range = self.vertex_allocator.alloc(
            upload_job.vertices.len() as u64,
            std::mem::align_of::<PNUJWVertex>() as u64,
        )?;
        let index_range = self.index_allocator.alloc(
            upload_job.indices.len() as u64,
            std::mem::align_of::<u16>() as u64,
        )?;
        queue.write_buffer(
            &self.vertex_buffer,
            vertex_range.start,
            bytemuck::cast_slice(upload_job.vertices),
        );

        queue.write_buffer(
            &self.index_buffer,
            index_range.start,
            bytemuck::cast_slice(upload_job.indices),
        );

        Some(GPUMeshHandle {
            vertex: vertex_range,
            index: index_range,
            count: upload_job.indices.len() as u64,
        })
    }
}

struct DrawItem<'v> {
    mesh_id: u32,
    vertex_slice: BufferSlice<'v>,
    index_slice: BufferSlice<'v>,
    index_count: u32,
}

struct RenderView<'v> {
    items: Vec<DrawItem<'v>>,
}

struct RenderGroup<'buffer, V: ModelVertex, I: IndexType> {
    v: PhantomData<V>,
    i: PhantomData<I>,
    pipeline: wgpu::RenderPipeline,
    views: Vec<RenderView<'buffer>>,
}

impl<'g, V: ModelVertex> RenderGroup<'g, V, u16> {
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
            v: PhantomData::<V>,
            i: PhantomData::<u16>,
            pipeline,
            views: Vec::new(),
        }
    }
}

pub struct Renderer<'group> {
    pnujw_mesh_arena: GPUMeshArena<PNUJWVertex, u16>,
    pnu_mesh_arena: GPUMeshArena<PNUVertex, u16>,
    pnujw_render_group: Option<RenderGroup<'group, PNUJWVertex, u16>>,
    pnu_render_group: Option<RenderGroup<'group, PNUVertex, u16>>,
}

impl Renderer<'_> {
    pub fn render(
        &mut self,
        config: &AppConfig,
        world_update_deltas: Vec<WorldUpdateDelta>,
    ) -> Result<(), wgpu::SurfaceError> {
        for delta in world_update_deltas {
            self.process_update_delta(config, delta);
        }
        if let Some(ref pnujw_render_group) = self.pnujw_render_group {
            Self::render_PNUJW(pnujw_render_group, config).unwrap();
        }
        if let Some(ref pnu_render_group) = self.pnu_render_group {
            todo!();
        }

        Ok(())
    }

    fn process_update_delta(&mut self, config: &AppConfig, delta: WorldUpdateDelta) {
        match delta {
            WorldUpdateDelta::EntityDidLoad(handle) => {
                let pnujw_vertices = Vec::<PNUJWVertex>::new();
                let pnujw_ref = &pnujw_vertices[..];
                let pnu_vertices = Vec::<PNUVertex>::new();
                let pnu_ref = &pnu_vertices[..];
                let indices = Vec::<u16>::new();
                let indices_ref = &indices[..];

                if !pnujw_ref.is_empty() {
                    self.ensure_render_group::<PNUJWVertex, u16>(
                        &config.device,
                        &wgpu::TextureFormat::R8Unorm,
                    )
                    .unwrap();
                    let upload_job = UploadMeshJob {
                        vertices: pnujw_ref,
                        indices: indices_ref,
                    };
                    let gpu_mesh_handle = self
                        .pnujw_mesh_arena
                        .upload_mesh(upload_job, &config.queue)
                        .unwrap();
                }
                if !pnu_ref.is_empty() {
                    self.ensure_render_group::<PNUVertex, u16>(
                        &config.device,
                        &wgpu::TextureFormat::R8Unorm,
                    )
                    .unwrap();

                    let upload_job = UploadMeshJob {
                        vertices: pnu_ref,
                        indices: indices_ref,
                    };
                    let gpu_mesh_handle =
                        self.pnu_mesh_arena.upload_mesh(upload_job, &config.queue);
                }

                todo!()
            }
        }
    }
    fn render_PNUJW(
        pnujw: &RenderGroup<PNUJWVertex, u16>,
        config: &AppConfig,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = config.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = config
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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

        render_pass.set_pipeline(&pnujw.pipeline);
        for a in pnujw.views.iter() {
            for d in a.items.iter() {
                // render pass draw
            }
        }
        todo!();

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
            if self.pnujw_render_group.is_none() {
                self.pnujw_render_group =
                    Some(RenderGroup::<PNUJWVertex, u16>::new(device, *format));
            }
            return Ok(());
        } else if v == TypeId::of::<PNUJWVertex>() && i == TypeId::of::<u16>() {
            if self.pnu_render_group.is_none() {
                self.pnu_render_group = Some(RenderGroup::<PNUVertex, u16>::new(device, *format));
            }
            return Ok(());
        } else {
            return Err(RendererError::UndefinedRenderGroup(v, i));
        }
    }

    pub(super) fn new(device: &wgpu::Device) -> Self {
        let vertex_size: u64 = 16 * 1024 * 1024;
        Renderer {
            pnujw_mesh_arena: GPUMeshArena::new(device, vertex_size, vertex_size / 4),
            pnujw_render_group: None,
            pnu_mesh_arena: GPUMeshArena::new(device, vertex_size, vertex_size / 4),
            pnu_render_group: None,
        }
    }
}

pub(super) enum RenderDelta {
    NewRenderable,
}
