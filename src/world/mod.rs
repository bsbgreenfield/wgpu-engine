use std::fmt::Display;

use crate::{
    asset_manager_new::{AssetHandle, AssetLoadError},
    world::{
        components::{MeshAcessor, RigidAnimationMode},
        entity_manager::{EntityHandle, EntityManagerError},
    },
};

pub mod camera;
pub mod components;
pub mod entity_manager;
mod index_arena;
pub mod instance_manager;
pub(super) mod load_queue;
pub mod scene;
pub mod world;
#[derive(Debug)]
pub enum WorldInitError {
    AssetFailure(AssetLoadError),
    EntityFailure(EntityManagerError),
}

#[derive(Debug)]
pub enum WorldUpdateError {
    AssetLoadFailure(AssetLoadError),
    AssetLoadNotComplete(AssetHandle),
    EntityLoadNotFound(EntityHandle),
    EntityLoadNotComplete(EntityHandle),
    EntityLoadFailed(EntityHandle),
    EntityLoadAlreadyEnqeued(EntityHandle),
    InstanceSpawnFailure,
}

impl Display for WorldUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AssetLoadFailure(err) => write!(
                f,
                "World update failed due to an asset load failure: {}",
                err
            ),
            Self::AssetLoadNotComplete(handle) => write!(
                f,
                "This asset with handle {:?} is not yet loaded, and not ready for use ",
                handle
            ),
            Self::EntityLoadNotFound(handle) => {
                write!(f, "Entity with handle {:?} does not exist", handle)
            }
            Self::EntityLoadNotComplete(handle) => write!(
                f,
                "The entity with handle {:?} is not yet loaded, and not ready for use ",
                handle
            ),
            Self::EntityLoadFailed(handle) => write!(f, "Entity load failed for {:?}", handle),
            Self::EntityLoadAlreadyEnqeued(handle) => write!(
                f,
                "Entity with handle {:?} was already enqueued for loading!",
                handle
            ),
            Self::InstanceSpawnFailure => f.write_str("failed to upload instance"),
        }
    }
}

impl std::error::Error for WorldUpdateError {}

impl From<AssetLoadError> for WorldUpdateError {
    fn from(value: AssetLoadError) -> Self {
        Self::AssetLoadFailure(value)
    }
}

impl From<AssetLoadError> for WorldInitError {
    fn from(value: AssetLoadError) -> Self {
        Self::AssetFailure(value)
    }
}
impl From<EntityManagerError> for WorldInitError {
    fn from(value: EntityManagerError) -> Self {
        Self::EntityFailure(value)
    }
}

#[derive(Default)]
pub struct InstanceUploadQuery<'a> {
    pub mesh_accesor: Option<&'a MeshAcessor>,
    pub rigid_animation_mode: Option<&'a RigidAnimationMode>,
}
