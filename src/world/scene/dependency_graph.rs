use std::{collections::btree_map::Range, fmt::Display, iter::Copied};

use crate::world::{
    RenderKey,
    entity_manager::entity_manager::EntityManager,
    scene::{SceneId, SceneLoadLevel, SceneNew},
};

#[derive(Debug)]
pub enum DependencyGraphError {
    InvalidChild,
    ChildNotFound,
    SceneNotFound,
}

impl std::error::Error for DependencyGraphError {}
impl Display for DependencyGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChild => f.write_str("invalid child node!"),
            Self::ChildNotFound => {
                f.write_str("could not find the dependency, although it is listed as a child")
            }
            Self::SceneNotFound => f.write_str("could not find this scene"),
        }
    }
}

struct SceneNode {
    children: Vec<SceneNode>,
    id: SceneId,
    rc: (usize, usize),
    entities: Vec<usize>,
    level: SceneLoadLevel,
}

struct EntityNode {
    rc: (usize, usize),
    assets: Vec<usize>,
}

struct AssetNode {
    rc: (usize, usize),
}

enum LoadLevelChangeResult {
    Changed,
    Unchanged,
}

fn get_max_level(rc: &(usize, usize)) -> SceneLoadLevel {
    if rc.1 > 0 {
        return SceneLoadLevel::GPU;
    } else if rc.0 > 0 {
        return SceneLoadLevel::CPU;
    } else {
        return SceneLoadLevel::NotLoaded;
    }
}
trait DepNode {
    fn set_level(
        &mut self,
        new_level: SceneLoadLevel,
        last_level: SceneLoadLevel,
    ) -> LoadLevelChangeResult {
        let rc = self.get_rc();
        let old_max = get_max_level(rc);
        match (new_level, last_level) {
            (SceneLoadLevel::CPU, SceneLoadLevel::CPU)
            | (SceneLoadLevel::GPU, SceneLoadLevel::GPU)
            | (SceneLoadLevel::NotLoaded, SceneLoadLevel::NotLoaded) => {}
            (SceneLoadLevel::CPU, SceneLoadLevel::GPU) => {
                rc.1 -= 1;
                rc.0 += 1;
            }
            (SceneLoadLevel::CPU, SceneLoadLevel::NotLoaded) => {
                rc.0 += 1;
            }
            (SceneLoadLevel::GPU, SceneLoadLevel::CPU) => {
                rc.0 -= 1;
                rc.1 += 1;
            }
            (SceneLoadLevel::GPU, SceneLoadLevel::NotLoaded) => {
                rc.1 += 1;
            }
            (SceneLoadLevel::NotLoaded, SceneLoadLevel::CPU) => {
                rc.0 -= 1;
            }
            (SceneLoadLevel::NotLoaded, SceneLoadLevel::GPU) => {
                rc.1 -= 1;
            }
        }
        let new_max = get_max_level(rc);
        if old_max != new_max {
            return LoadLevelChangeResult::Changed;
        } else {
            return LoadLevelChangeResult::Unchanged;
        }
    }

    fn get_rc(&mut self) -> &mut (usize, usize);
}

impl DepNode for EntityNode {
    fn get_rc(&mut self) -> &mut (usize, usize) {
        &mut self.rc
    }
}
impl DepNode for SceneNode {
    fn get_rc(&mut self) -> &mut (usize, usize) {
        &mut self.rc
    }
}
impl DepNode for AssetNode {
    fn get_rc(&mut self) -> &mut (usize, usize) {
        &mut self.rc
    }
}

pub struct DependencyGraph {
    roots: Vec<SceneNode>,
    entities: Vec<EntityNode>,
    assets: Vec<AssetNode>,
}

impl DependencyGraph {
    fn find_scene<'a>(root: &'a mut SceneNode, scene_id: SceneId) -> Option<&'a mut SceneNode> {
        if root.id == scene_id {
            return Some(root);
        }
        if root.children.is_empty() {
            return None;
        }

        for child in root.children.iter_mut() {
            if let Some(res) = Self::find_scene(child, scene_id) {
                return Some(res);
            }
        }
        None
    }

    pub fn set_load_level(
        &mut self,
        scene_id: SceneId,
        new_level: SceneLoadLevel,
    ) -> Result<(), DependencyGraphError> {
        let Self {
            roots,
            entities,
            assets,
        } = self;
        let maybe_scene = roots.iter_mut().find_map(|r| {
            return Self::find_scene(r, scene_id);
        });

        let mut scene_queue = Vec::<&mut SceneNode>::new();
        let last_level = if let Some(scene_node) = maybe_scene {
            println!("scene found = {:?}, expected {:?}", scene_node.id, scene_id);
            let l = scene_node.level;
            if new_level == l {
                return Ok(());
            }
            scene_node.level = new_level;
            scene_node.set_level(new_level, l);
            scene_queue.push(scene_node);
            l
        } else {
            return Err(DependencyGraphError::SceneNotFound);
        };
        while !scene_queue.is_empty() {
            let curr = scene_queue.pop().unwrap();
            for entity_idx in curr.entities.iter() {
                let entity_node = entities.get_mut(*entity_idx).expect("should exist");
                match entity_node.set_level(new_level, last_level) {
                    LoadLevelChangeResult::Changed => {
                        for asset in entity_node.assets.iter() {
                            let asset_node = assets.get_mut(*asset).unwrap();
                            asset_node.set_level(new_level, last_level);
                        }
                    }
                    LoadLevelChangeResult::Unchanged => {}
                }
            }
            for child in curr.children.iter_mut() {
                child.set_level(new_level, last_level);
                scene_queue.push(child);
            }
        }

        Ok(())
    }
    pub fn add_scene(
        &mut self,
        scene: &SceneNew,
        entity_manager: &EntityManager,
    ) -> Result<(), DependencyGraphError> {
        let mut new_root = SceneNode {
            children: vec![],
            id: scene.id,
            rc: (0, 0),
            entities: vec![],
            level: SceneLoadLevel::NotLoaded,
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
            new_root.entities.push(child_entity.0.0 as usize);
            if self.entities.get(child_entity.0.0 as usize).is_none() {
                if self.entities.len() <= child_entity.0.0 as usize {
                    self.entities
                        .resize_with(child_entity.0.0 as usize + 1, || EntityNode {
                            rc: (0, 0),
                            assets: vec![],
                        });
                }
                let entity_node = self
                    .entities
                    .get_mut(child_entity.0.0 as usize)
                    .expect("just added entity, where is it?");
                for asset_handle in entity_manager.rbcs_of(child_entity.0) {
                    entity_node.assets.push(asset_handle.as_key() as usize);
                    if self.assets.len() <= asset_handle.as_key() as usize {
                        self.assets
                            .resize_with((asset_handle.as_key() + 1) as usize, || AssetNode {
                                rc: (0, 0),
                            });
                    }
                }
            }
        }

        self.roots.push(new_root);
        Ok(())
    }

    pub fn new() -> Self {
        Self {
            roots: vec![],
            entities: vec![],
            assets: vec![],
        }
    }
}

#[cfg(test)]
impl DependencyGraph {
    pub(super) fn root_ids(&self) -> Vec<SceneId> {
        self.roots.iter().map(|r| r.id).collect()
    }

    /// children of a top level scene, or None if that scene isn't a root
    pub(super) fn child_ids_of(&self, id: SceneId) -> Option<Vec<SceneId>> {
        self.roots
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.children.iter().map(|c| c.id).collect())
    }

    pub(super) fn entity_node_count(&self) -> usize {
        self.entities.len()
    }

    pub(super) fn asset_node_count(&self) -> usize {
        self.assets.len()
    }

    /// rc of a scene anywhere in the forest, or None if it isn't in the graph
    pub(super) fn scene_rc(&self, id: SceneId) -> Option<(usize, usize)> {
        fn find(nodes: &[SceneNode], id: SceneId) -> Option<(usize, usize)> {
            for node in nodes {
                if node.id == id {
                    return Some(node.rc);
                }
                if let Some(rc) = find(&node.children, id) {
                    return Some(rc);
                }
            }
            None
        }
        find(&self.roots, id)
    }

    pub(super) fn entity_rc(
        &self,
        entity: crate::common::entity::EntityHandle,
    ) -> Option<(usize, usize)> {
        self.entities.get(entity.0 as usize).map(|n| n.rc)
    }

    pub(super) fn asset_rc(
        &self,
        asset: crate::asset_manager::AssetHandle,
    ) -> Option<(usize, usize)> {
        self.assets.get(asset.as_key() as usize).map(|n| n.rc)
    }
}
