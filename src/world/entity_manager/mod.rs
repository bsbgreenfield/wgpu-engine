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

#[derive(Debug)]
pub(crate) struct MaterialBinding {
    alloc_handle: GPUAllocationHandle,
    pub index: u32,
}

// TODO: both mesh renderables and material palette are a Vec<(alloc, data)>
// in anticipation of a future in which multiple mesh components and material components
// are allowed, which is not currently the case
pub(crate) struct Renderables {
    pub instance_handle: InstanceHandle,
    pub(crate) mesh_renderables: Vec<(GPUAllocationHandle, MeshRenderables)>,
    pub animations: Option<EntityAnimationData>,
    pub material_palette: Vec<(GPUAllocationHandle, Vec<u32>)>,
}
impl Error for EntityManagerError {}
