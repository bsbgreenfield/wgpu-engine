use std::fmt::Display;

use crate::{
    app::renderer::{RenderError, RenderUpdateError},
    world::WorldUpdateError,
};

pub mod app;
pub mod app_config;
pub mod app_state;
pub mod renderer;

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
