use wgpu::BufferSlice;

use crate::{
    app::{
        app_config::AppConfig,
        render::{
            GPUMeshHandle, Instruction, OpaquePass, VMValue, arena::GPUMeshArena,
            render_group::RenderGroup,
        },
    },
    asset_manager::asset_manager::{AssetHandle, LoadedAsset},
    util::types::{ModelVertex, PNUJWVertex, PNUVertex},
};

pub enum RenderUpdateDelta {
    AssetGPULoaded(GPUMeshHandle),
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

pub struct Renderer<'group> {
    opaque_arena: GPUMeshArena,
    opaque_render_group: RenderGroup<'group, OpaquePass>,
}

impl<'group> Renderer<'group> {
    pub fn render(&mut self, config: &AppConfig) -> Result<(), wgpu::SurfaceError> {
        todo!("RENDER");
    }

    pub(in crate::app) fn update(
        &mut self,
        constants: Vec<VMValue>,
        ops: Vec<Instruction>,
        queue: &wgpu::Queue,
    ) -> Vec<RenderUpdateDelta> {
        self.interpret(constants, ops, queue)
    }

    pub fn new(config: &AppConfig) -> Self {
        let vertex_size: u64 = 16 * 1024 * 1024;
        Renderer {
            opaque_render_group: RenderGroup::<OpaquePass>::new(
                config,
                include_str!("../../shader.wgsl"), //remove hardcoded shader path
            ),
            opaque_arena: GPUMeshArena::new(
                Some("opaque arena"),
                &config.device,
                vertex_size,
                vertex_size / 4,
            ),
        }
    }

    pub(super) fn generate_draw_calls(&mut self) {
        todo!()
    }

    pub(super) fn set_la_data(
        &mut self,
        la: &LoadedAsset,
        queue: &wgpu::Queue,
    ) -> Option<GPUMeshHandle> {
        let gltf_data = &la.gltf_mesh_data;
        // TODO: return GPU handle
        self.opaque_arena.upload_mesh(
            la.handle,
            Some(&gltf_data.pnujw_vertices),
            Some(&gltf_data.pnu_vertices),
            Some(&gltf_data.indices),
            queue,
        )
    }
}

pub(super) enum RenderDelta {
    NewRenderable,
}
