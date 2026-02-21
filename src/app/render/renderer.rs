use std::mem::MaybeUninit;

use wgpu::BufferSlice;

use crate::{
    app::{
        app_config::AppConfig,
        render::{Instruction, OpaquePass, RenderGroup, VMValue, arena::GPUMeshArena},
    },
    asset_manager::asset_manager::LoadedAsset,
    util::types::{PNUJWVertex, PNUVertex},
};

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
    pnujw_mesh_arena: GPUMeshArena<PNUJWVertex, u16>,
    pnu_mesh_arena: GPUMeshArena<PNUVertex, u16>,
    opaque_render_group: RenderGroup<'group, OpaquePass>,
}

impl<'group> Renderer<'group> {
    pub fn render(&mut self, config: &AppConfig) -> Result<(), wgpu::SurfaceError> {
        todo!("RENDER");
    }

    pub(in crate::app) fn update(&mut self, constants: Vec<VMValue>, ops: Vec<Instruction>) {
        self.interpret(constants, ops);
    }

    pub fn new(config: &AppConfig) -> Self {
        let vertex_size: u64 = 16 * 1024 * 1024;
        Renderer {
            pnujw_mesh_arena: GPUMeshArena::new(&config.device, vertex_size, vertex_size / 4),
            opaque_render_group: RenderGroup::<OpaquePass>::new(
                config,
                include_str!("../../shader.wgsl"), //remove hardcoded shader path
            ),
            pnu_mesh_arena: GPUMeshArena::new(&config.device, vertex_size, vertex_size / 4),
        }
    }
}

pub(super) enum RenderDelta {
    NewRenderable,
}
