use std::collections::{HashMap, HashSet};

use crate::{
    asset_manager::{AssetHandle, asset_manager::AssetManager},
    common::entity::EntityHandle,
    world::{
        RenderKey,
        entity_manager::entity_manager::EntityManager,
        scene::{SceneId, SceneLoadLevel, SceneNew, dependency_graph::DependencyGraphError},
    },
};

pub struct SweepResult {
    pub required_assets: HashMap<AssetHandle, SceneLoadLevel>,
    pub achieved_scenes: HashMap<SceneId, SceneLoadLevel>,
}

struct SceneNode {
    children: Vec<SceneId>,
    entities: Vec<EntityHandle>,
    assets: HashSet<AssetHandle>,
}

struct EntityNode {
    assets: Vec<AssetHandle>,
    known: bool,
}

#[derive(Default)]
pub struct DependencyGraphNew {
    scenes: HashMap<SceneId, SceneNode>,
}

impl DependencyGraphNew {
    pub fn required_assets_of(&self, scene_id: SceneId) -> &HashSet<AssetHandle> {
        &self.scenes.get(&scene_id).unwrap().assets
    }
    pub fn holders_of(&self, asset: &AssetHandle) -> impl Iterator<Item = SceneId> + '_ {
        let asset = *asset;
        self.scenes
            .iter()
            .filter(move |(_, node)| node.assets.contains(&asset))
            .map(|(id, _)| *id)
    }
    pub fn sweep(
        &self,
        scenes: impl Iterator<Item = (SceneId, SceneLoadLevel)>,
        asset_manager: &AssetManager,
    ) -> Result<SweepResult, DependencyGraphError> {
        let mut required_assets = HashMap::new();
        let mut achieved_scenes = HashMap::new();

        for (scene_id, requested) in scenes {
            let node = self
                .scenes
                .get(&scene_id)
                .ok_or(DependencyGraphError::SceneNotFound)?;
            // a scene with no entities of its own is trivially as loaded as it can get
            let mut achieved = SceneLoadLevel::GPU;
            for asset in node.assets.iter() {
                let slot = required_assets
                    .entry(*asset)
                    .or_insert(SceneLoadLevel::NotLoaded);
                *slot = (*slot).max(requested);

                let residency = asset_manager
                    .res_level_of(asset)
                    .map_err(|_| DependencyGraphError::ChildNotFound)?;
                achieved = achieved.min(SceneLoadLevel::from(&residency));
            }
            achieved_scenes.insert(scene_id, achieved);
        }
        Ok(SweepResult {
            required_assets,
            achieved_scenes,
        })
    }
    pub fn add_scene(
        &mut self,
        scene: &SceneNew,
        entity_manager: &EntityManager,
    ) -> Result<(), DependencyGraphError> {
        let mut children: Vec<SceneId> = Vec::new();
        for child in scene.desc.children.iter() {
            children.push(*child);
        }

        let mut assets = HashSet::new();
        for entity in scene.desc.entities.iter() {
            assets.extend(entity_manager.rbcs_of(*entity));
        }
        let new_root = SceneNode {
            children,
            entities: vec![],
            assets,
        };

        self.scenes.insert(scene.id, new_root);

        Ok(())
    }
    pub fn children_of(&self, scene_id: SceneId) -> &[SceneId] {
        self.scenes
            .get(&scene_id)
            .map(|s| s.children.as_slice())
            .unwrap_or(&[])
    }
}
