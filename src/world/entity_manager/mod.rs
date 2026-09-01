use std::{error::Error, fmt::Display};

use crate::{
    animation::EntityAnimationData, asset_manager::MeshRenderables,
    common::instance::InstanceHandle, renderer::GPUAllocationHandle,
};

pub mod components;
pub mod entity_manager;
mod tests;

#[derive(Debug)]
pub enum EntityManagerError {
    MaxEntitiesExceeded,
    InvalidInitialization,
    UploadJobFail,
    RenderableFetchError(String),
}

impl Display for EntityManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return f.write_str(&self.to_string());
    }
}
pub(crate) struct Renderables {
    pub instance_handle: InstanceHandle,
    pub(crate) mesh_renderables: Vec<(GPUAllocationHandle, MeshRenderables)>,
    pub animations: Option<EntityAnimationData>,
}
impl Error for EntityManagerError {}
