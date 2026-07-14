use std::{fmt::Display, sync::Arc};

use crate::{
    asset_manager::{AssetHandle, AssetLoadError},
    renderer::{RenderError, RenderUpdateError},
    util::types::{PNUJWVertex, PNUVertex, VIndex},
    world::WorldUpdateError,
};

pub mod app;
pub mod app_config;
pub mod app_state;

#[derive(Debug, Clone)]
pub struct GPUAssetUploadJob {
    pub asset_handle: AssetHandle,
    pub pnu_vertices: Option<Arc<[PNUVertex]>>,
    pub pnujw_vertices: Option<Arc<[PNUJWVertex]>>,
    pub indices: Option<Arc<[VIndex]>>,
}

impl GPUAssetUploadJob {
    pub fn new(
        asset_handle: AssetHandle,
        pnu_vertices: Option<Arc<[PNUVertex]>>,
        pnujw_vertices: Option<Arc<[PNUJWVertex]>>,
        indices: Option<Arc<[VIndex]>>,
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
