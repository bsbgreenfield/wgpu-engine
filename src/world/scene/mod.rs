pub mod builder;
pub mod dependency_graph;
pub mod manager;
pub mod scene;
mod test;
pub mod util;
use crate::{
    asset_manager::AssetResidency,
    common::entity::EntityHandle,
    world::{
        instance_manager::archetypes::Archetype,
        scene::scene::{SceneDesc, SceneRuntime},
    },
};
use std::fmt::Debug;

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct SceneId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
pub enum SceneLoadLevel {
    NotLoaded,
    PendingCPU,
    CPU,
    PendingGPU,
    GPU,
}
impl Default for SceneLoadLevel {
    fn default() -> Self {
        Self::NotLoaded
    }
}

impl From<&AssetResidency> for SceneLoadLevel {
    fn from(value: &AssetResidency) -> Self {
        match value {
            AssetResidency::PendingUnloadGPU => SceneLoadLevel::GPU,
            AssetResidency::PendingGPU(_) => SceneLoadLevel::PendingGPU,
            AssetResidency::PendingCPU => SceneLoadLevel::PendingCPU,
            AssetResidency::Registered => SceneLoadLevel::NotLoaded,
            AssetResidency::CPU(_) => SceneLoadLevel::CPU,
            AssetResidency::GPU(_, _) => SceneLoadLevel::GPU,
        }
    }
}

pub enum SceneEvent {
    LoadLevelChanged(SceneLoadLevel, SceneLoadLevel),
    Spawn(Vec<(EntityHandle, Box<dyn Archetype>)>),
    SpawnNew,
}

impl SceneEvent {
    fn priority(&self) -> usize {
        match self {
            Self::LoadLevelChanged(_, _) => 1,
            Self::Spawn(_) => 0,
            Self::SpawnNew => 0,
        }
    }
}

impl PartialEq for SceneEvent {
    fn eq(&self, other: &Self) -> bool {
        self.priority() == other.priority()
    }
}

impl Eq for SceneEvent {}

impl PartialOrd for SceneEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SceneEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority().cmp(&other.priority())
    }
}
impl Debug for SceneEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadLevelChanged(arg0, arg1) => f
                .debug_tuple("LoadLevelChanged")
                .field(arg0)
                .field(arg1)
                .finish(),
            Self::Spawn(arg0) => write!(f, "Spawn {} entities ", arg0.len()),
            Self::SpawnNew => write!(f, "SpawnNew"),
        }
    }
}

pub struct Scene {
    pub id: SceneId,
    desc: SceneDesc,
    runtime: SceneRuntime,
}
