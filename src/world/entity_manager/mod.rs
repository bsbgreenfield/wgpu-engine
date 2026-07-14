use std::{error::Error, fmt::Display};

use crate::{
    animation::animation::EntityAnimationData, asset_manager::MeshRenderables,
    renderer::GPUAllocationHandle, world::instance_manager::InstanceHandle,
};

pub mod components;
pub mod entity_manager;
mod tests;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityHandle(pub u16);

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
pub struct Renderables {
    pub instance_handle: InstanceHandle,
    pub mesh_renderables: Vec<(GPUAllocationHandle, MeshRenderables)>,
    pub animations: Option<EntityAnimationData>,
}
impl Error for EntityManagerError {}
