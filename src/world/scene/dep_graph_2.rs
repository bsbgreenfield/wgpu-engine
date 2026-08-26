use std::collections::HashMap;

use crate::{
    asset_manager::{AssetHandle, asset_manager::AssetManager},
    common::entity::EntityHandle,
    world::{
        RenderKey,
        entity_manager::entity_manager::EntityManager,
        scene::{SceneId, SceneLoadLevel, SceneNew, dependency_graph::DependencyGraphError},
    },
};

struct SceneNode {
    children: Vec<SceneId>,
    entities: Vec<EntityHandle>,
}

struct EntityNode {
    assets: Vec<AssetHandle>,
    known: bool,
}

#[derive(Default)]
pub struct DependencyGraphNew {
    scenes: HashMap<SceneId, SceneNode>,
    entities: Vec<EntityNode>,
    roots: Vec<SceneId>,
}

impl DependencyGraphNew {
    pub fn add_scene(
        &mut self,
        scene: &SceneNew,
        entity_manager: &EntityManager,
    ) -> Result<(), DependencyGraphError> {
        let mut new_root = SceneNode {
            children: vec![],
            entities: vec![],
        };
        for child in scene.desc.children.iter() {
            let pos = self
                .roots
                .iter()
                .position(|r| r == child)
                .ok_or(DependencyGraphError::ChildNotFound)?;

            let child_node = self.roots.swap_remove(pos);
            new_root.children.push(child_node);
        }

        for child_entity in scene.desc.entities.iter() {
            new_root.entities.push(*child_entity);
            if self.entities.get(child_entity.0 as usize).is_none() {
                if self.entities.len() <= child_entity.0 as usize {
                    self.entities
                        .resize_with(child_entity.0 as usize + 1, || EntityNode {
                            assets: vec![],
                            known: false,
                        });
                }
                let entity_node = self
                    .entities
                    .get_mut(child_entity.0 as usize)
                    .expect("just added entity, where is it?");
                for asset_handle in entity_manager.rbcs_of(*child_entity) {
                    entity_node.assets.push(asset_handle);
                }
            }
        }

        self.roots.push(scene.id);
        Ok(())
    }
    pub fn children_of(&self, scene_id: SceneId) -> &[SceneId] {
        self.scenes
            .get(&scene_id)
            .map(|s| s.children.as_slice())
            .unwrap_or(&[])
    }
    pub fn resolve(
        &self,
        scene_id: SceneId,
        requested: SceneLoadLevel,
        asset_manager: &AssetManager,
        requests: &mut HashMap<AssetHandle, SceneLoadLevel>,
        visited: &mut HashMap<SceneId, SceneLoadLevel>,
    ) -> Result<SceneLoadLevel, DependencyGraphError> {
        if let Some(level) = visited.get(&scene_id) {
            return Ok(*level);
        }

        visited.insert(scene_id, SceneLoadLevel::GPU);

        let node = self
            .scenes
            .get(&scene_id)
            .ok_or(DependencyGraphError::SceneNotFound)?;

        // initialize to GPU, which is trivially true if this scene has no deps
        let mut level = SceneLoadLevel::GPU;

        // reduce level to equal the minimum level of this scenes descendants
        for child in node.children.iter() {
            let child_level = self.resolve(*child, requested, asset_manager, requests, visited)?;
            level = level.min(child_level);
        }

        for entity in node.entities.iter() {
            for asset in self
                .entities
                .get(entity.0 as usize)
                .map(|e| e.assets.as_slice())
                .unwrap_or(&[])
            {
                let actual_asset_residency = asset_manager
                    .res_level_of(asset)
                    .map_err(|_| DependencyGraphError::ChildNotFound)?;
                let asset_level = SceneLoadLevel::from(&actual_asset_residency);

                // this asset needs to be loaded. its load level
                // should be the maximum of all deps that have requested it
                if asset_level < requested {
                    let request = requests.entry(*asset).or_insert(requested);
                    *request = (*request).max(requested);
                }
                // reduce level again to be the minimum asset level
                level = level.min(asset_level);
            }
        }
        visited.insert(scene_id, level);
        Ok(level)
    }
}
