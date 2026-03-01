use std::{any::TypeId, marker::PhantomData, ops::Range, sync::Arc};

use wgpu::BufferSlice;

use crate::{
    app::app_config::AppConfig,
    asset_manager::asset_manager::{AssetHandle, LoadedAsset},
    util::types::{IndexType, Mat4F32, ModelVertex},
};

mod arena;
mod opaque_pass;
mod render_group;
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

pub(super) struct OpaquePass;
#[derive(Debug)]
enum RendererError {
    UndefinedRenderGroup(TypeId, TypeId),
}

pub struct GPUMeshHandle {
    pub handle: AssetHandle,
    vertex_pnu: Range<u64>,
    vertex_pnujw: Range<u64>,
    index: Range<u64>,
    count: u64,
}
