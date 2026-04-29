use std::fmt::Display;

use crate::{
    app::renderer::{RenderError, RenderUpdateError},
    asset_manager_new::{AssetHandle, AssetLoadError},
    util::types::{PNUJWVertex, PNUVertex, VIndex},
    world::WorldUpdateError,
};

pub mod app;
pub mod app_config;
pub mod app_state;
pub mod renderer;

#[derive(Debug, Clone)]
pub struct GPUAssetUploadJob<'a> {
    asset_handle: &'a AssetHandle,
    pnu_vertices: Option<&'a [PNUVertex]>,
    pnujw_vertices: Option<&'a [PNUJWVertex]>,
    indices: Option<&'a [VIndex]>,
}

impl<'a> GPUAssetUploadJob<'a> {
    pub fn new(
        asset_handle: &'a AssetHandle,
        pnu_vertices: Option<&'a [PNUVertex]>,
        pnujw_vertices: Option<&'a [PNUJWVertex]>,
        indices: Option<&'a [VIndex]>,
    ) -> Result<Self, AssetLoadError> {
        if pnu_vertices.is_none() && pnujw_vertices.is_none() {
            return Err(AssetLoadError::NoVertexData);
        }
        Ok(Self {
            asset_handle,
            pnu_vertices,
            pnujw_vertices,
            indices,
        })
    }
}

#[allow(unused)]
#[derive(Debug)]
pub enum FrameError {
    UpdateError(WorldUpdateError),
    SurfaceError(wgpu::SurfaceError),
    RenderUpdateError(RenderUpdateError),
    RenderError(RenderError),
}

impl Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UpdateError(err) => err.fmt(f),
            Self::SurfaceError(err) => err.fmt(f),
            Self::RenderUpdateError(err) => err.fmt(f),
            Self::RenderError(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<WorldUpdateError> for FrameError {
    fn from(value: WorldUpdateError) -> Self {
        FrameError::UpdateError(value)
    }
}

impl From<RenderUpdateError> for FrameError {
    fn from(value: RenderUpdateError) -> Self {
        FrameError::RenderUpdateError(value)
    }
}

impl From<RenderError> for FrameError {
    fn from(value: RenderError) -> Self {
        FrameError::RenderError(value)
    }
}
