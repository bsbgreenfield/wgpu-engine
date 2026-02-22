use std::{any::TypeId, marker::PhantomData, ops::Range, sync::Arc};

use wgpu::BufferSlice;

use crate::{
    app::app_config::AppConfig,
    asset_manager::asset_manager::LoadedAsset,
    util::types::{IndexType, Mat4F32, ModelVertex},
};

mod arena;
mod opaque_pass;
pub mod renderer;
mod renderer_vm;

#[derive(Clone, Copy)]
pub(super) enum Instruction {
    Op(Operations),
    Byte(u8),
    ConstIdx(u8),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Operations {
    AddEntity,
    MoveEntity,
}

pub(super) enum VMValue<'frame> {
    Transform(Mat4F32),
    LoadedAsset(&'frame LoadedAsset),
}

impl<'frame> VMValue<'frame> {
    fn unwrap_loaded_asset(&self) -> &'frame LoadedAsset {
        match self {
            VMValue::LoadedAsset(la) => la,
            _ => panic!("value is not a loaded asset ref"),
        }
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

trait RenderGroupType {
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
    fn new(config: &AppConfig, shader_module: &str) -> Self {
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
}
pub(super) struct OpaquePass;
#[derive(Debug)]
enum RendererError {
    UndefinedRenderGroup(TypeId, TypeId),
}

struct UploadMeshJob<'j, V: ModelVertex, I: IndexType> {
    vertices: &'j [V],
    indices: &'j [I],
}

struct GPUMeshHandle {
    vertex: Range<u64>,
    index: Range<u64>,
    count: u64,
}
