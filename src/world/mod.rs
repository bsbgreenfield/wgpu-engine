use std::fmt::Display;

use crate::{
    app::GPUAssetUploadJob,
    asset_manager::{AssetHandle, AssetLoadError},
    common::{entity::EntityHandle, instance::InstanceHandle},
    renderer::{GPUAllocationHandle, GPUInstanceHandle, PrototypeHandle, RenderBytes},
    util::types::{AssetIndices, GPUTextureData},
    world::{
        entity_manager::EntityManagerError,
        instance_manager::{CopiedInstanceData, NewInstanceData},
        scene::manager::SceneManagerError,
    },
};

pub(super) mod bytecode_gen;
pub mod entity_manager;
pub mod instance_manager;
mod load_queue;
pub mod scene;
pub mod world;
#[derive(Debug)]
pub enum WorldInitError {
    AssetFailure(AssetLoadError),
    EntityFailure(EntityManagerError),
    SceneCreationFailure(SceneManagerError),
}

impl From<SceneManagerError> for WorldInitError {
    fn from(value: SceneManagerError) -> Self {
        return Self::SceneCreationFailure(value);
    }
}

#[derive(Clone)]
pub(crate) enum WorldUpdateDelta {
    NewEntitySpawn(NewInstanceData),
    EntityInstanceSpawn(CopiedInstanceData),
    AssetDidLoad(GPUAssetUploadJob),
    AssetUnload(AssetHandle, GPUAllocationHandle),
    InstanceDespawn(GPUInstanceHandle),
    ReleasePrototype(PrototypeHandle),
}

impl<'frame> std::fmt::Debug for WorldUpdateDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldUpdateDelta::NewEntitySpawn(_) => f.write_str("NewEntitySpawn"),
            WorldUpdateDelta::EntityInstanceSpawn(_) => f.write_str("EntityInstanceSpawn"),
            WorldUpdateDelta::AssetDidLoad(_) => f.write_str("AssetDidLoad"),
            WorldUpdateDelta::InstanceDespawn(handle) => write!(f, "despawn {:?}", handle),
            WorldUpdateDelta::AssetUnload(_asset_handle, alloc_handle) => {
                write!(f, "unload asset {:?}", alloc_handle)
            }
            WorldUpdateDelta::ReleasePrototype(p) => write!(f, "release prototype {p:?}"),
        }
    }
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
    InstancceNotFound(InstanceHandle),
    RenderablesNotAvailable(EntityHandle),
    SceneManagerError(SceneManagerError),
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
            Self::InstancceNotFound(handle) => write!(f, "instance not found: {:?}", handle),
            Self::RenderablesNotAvailable(handle) => write!(
                f,
                "In update render state, the entity with handle {:?} could not generate renderable data",
                handle
            ),
            Self::SceneManagerError(sme) => sme.fmt(f),
        }
    }
}

impl std::error::Error for WorldUpdateError {}

impl From<AssetLoadError> for WorldUpdateError {
    fn from(value: AssetLoadError) -> Self {
        Self::AssetLoadFailure(value)
    }
}

impl From<SceneManagerError> for WorldUpdateError {
    fn from(value: SceneManagerError) -> Self {
        Self::SceneManagerError(value)
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

pub struct InstanceResidency {
    pub group_id: u64,
    pub record_index: u32,
    pub bind_key: u32,
}

impl InstanceResidency {
    #[inline(always)]
    pub const fn is_pending(&self) -> bool {
        self.bind_key == u32::MAX
    }
    fn pending() -> Self {
        Self {
            group_id: 0,
            record_index: u32::MAX,
            bind_key: u32::MAX,
        }
    }
}

impl<T: bytemuck::Pod + Send + Sync> RenderBytes for Vec<T> {
    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}

impl RenderBytes for AssetIndices {
    fn as_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl RenderBytes for GPUTextureData {
    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.pixels)
    }
}
