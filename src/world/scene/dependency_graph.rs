use std::collections::btree_map::Range;

use crate::{
    asset_manager::AssetHandle,
    common::entity::EntityHandle,
    world::{
        RenderKey,
        entity_manager::entity_manager::EntityManager,
        scene::{SceneId, SceneLoadLevel, SceneNew},
    },
};

enum DependencyGraphError {
    InvalidChild,
    ChildNotFound,
}

struct SceneNode {
    children: Vec<SceneNode>,
    id: SceneId,
    rc: (usize, usize),
}

struct EntityNode {
    rc: (usize, usize),
}

struct AssetNode {
    rc: (usize, usize),
}

pub struct DependencyGraph {
    roots: Vec<SceneNode>,
    entities: Vec<EntityNode>,
    assets: Vec<AssetNode>,
}

impl DependencyGraph {
    pub fn add_scene(
        &mut self,
        scene: SceneNew,
        entity_manager: &EntityManager,
    ) -> Result<(), DependencyGraphError> {
        let mut new_root = SceneNode {
            children: vec![],
            id: scene.id,
            rc: (0, 0),
        };
        for child in scene.desc.children.iter() {
            let pos = self
                .roots
                .iter()
                .position(|r| r.id == *child)
                .ok_or(DependencyGraphError::ChildNotFound)?;

            let child_node = self.roots.swap_remove(pos);
            new_root.children.push(child_node);
        }

        for child_entity in scene.desc.entities.iter() {
            if self.entities.get(child_entity.0.0 as usize).is_none() {
                if self.entities.len() < child_entity.0.0 as usize {
                    self.entities
                        .resize_with(child_entity.0.0 as usize, || EntityNode { rc: (0, 0) });
                }
                for asset_handle in entity_manager.rbcs_of(child_entity.0) {
                    self.assets
                        .resize_with(asset_handle.as_key() as usize, || AssetNode { rc: (0, 0) });
                }
            }
        }

        Ok(())
    }
}
