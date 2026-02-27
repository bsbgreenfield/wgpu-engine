use std::{any::TypeId, marker::PhantomData, ops::Range};

use wgpu::BufferSlice;

use crate::{
    app::{
        app_config::AppConfig,
        render::{GPUMeshHandle, arena::GPUMeshArena},
    },
    asset_manager::gltf_assets::model_builder_new::GltfLoadResult,
    util::types::PNUJWVertex,
};

struct DrawItem<'v> {
    mesh_id: u32,
    vertex_slice: BufferSlice<'v>,
    index_slice: BufferSlice<'v>,
    index_count: u32,
}

pub(super) struct RenderView<'v> {
    items: Vec<DrawItem<'v>>,
}

impl RenderView<'_> {
    fn new() -> Self {
        Self {
            items: Vec::<DrawItem>::new(),
        }
    }
}

pub(super) trait RenderGroupType {
    fn create_pass<'pass>() -> wgpu::RenderPass<'pass>;
    fn create_pipelines(
        config: &AppConfig,
        shader: &wgpu::ShaderModule,
    ) -> Vec<wgpu::RenderPipeline>;
}

pub(super) struct RenderGroup<'buffer, P: RenderGroupType> {
    _render_pass: PhantomData<P>,
    pipelines: Vec<wgpu::RenderPipeline>,
    views: Vec<RenderView<'buffer>>,
}

impl<'rg, P: RenderGroupType> RenderGroup<'rg, P> {
    pub(super) fn new(config: &AppConfig, shader_module: &str) -> Self {
        let shader = config
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_module.into()),
            });

        let pipelines = P::create_pipelines(config, &shader);
        let views = Vec::<RenderView>::new();

        Self {
            _render_pass: PhantomData,
            pipelines,
            views,
        }
    }

    pub(super) fn create_mesh_view(
        &mut self,
        arena: &'rg GPUMeshArena,
        gltf_data: &GltfLoadResult,
        gpu_mesh_handle: &GPUMeshHandle,
    ) -> RenderView<'rg> {
        let mut draw_items = Vec::<DrawItem>::new();
        let pnu_offst = gpu_mesh_handle.vertex_pnu.start as u32;
        let arena_index_offset = gpu_mesh_handle.index.start as u32;
        for mesh_data in gltf_data.mesh_data.iter() {
            let mut mesh_id: u32;
            for mesh in mesh_data.meshes.iter() {
                mesh_id = mesh.id;
                for primitive in mesh.primitives.iter() {
                    let buffer = if primitive.vertex_type == TypeId::of::<PNUJWVertex>() {
                        &arena.pnujw_vertex_buffer
                    } else {
                        &arena.pnu_vertex_buffer
                    };
                    let vertex_range = Range {
                        start: (primitive.vertices.start + arena_vertex_offset) as u64,
                        end: (primitive.vertices.end + arena_vertex_offset) as u64,
                    };
                    let index_range = Range {
                        start: (primitive.indices.start + arena_index_offset) as u64,
                        end: (primitive.indices.end + arena_index_offset) as u64,
                    };
                    let draw_item = DrawItem {
                        vertex_slice: buffer.slice(vertex_range),
                        index_slice: buffer.slice(index_range),
                        mesh_id: mesh_id,
                        index_count: primitive.indices.len() as u32,
                    };
                    draw_items.push(draw_item);
                }
            }
        }

        RenderView { items: draw_items }
    }
}
