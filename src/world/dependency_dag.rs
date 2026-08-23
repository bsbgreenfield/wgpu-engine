use std::{collections::HashMap, fmt::Display};

use crate::{
    asset_manager::AssetHandle,
    common::entity::EntityHandle,
    world::{
        entity_manager::entity_manager::EntityManager,
        scene::{SceneId, SceneLoadLevel, scene::Scene},
    },
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
enum DependencyNode {
    Scene(SceneId),
    Entity(EntityHandle),
    Asset(AssetHandle),
}

#[derive(Debug)]
pub enum DependencyDAGError {
    NodeNotFound(String),
    InvalidParent(String),
    InvalidChild(String),
}
impl std::error::Error for DependencyDAGError {}
impl Display for DependencyDAGError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyDAGError::NodeNotFound(s) => f.write_str(s),
            DependencyDAGError::InvalidParent(s) => f.write_str(s),
            DependencyDAGError::InvalidChild(s) => f.write_str(s),
        }
    }
}

struct DependencyEntry {
    children: Vec<DependencyNode>,
    parents: Vec<DependencyNode>,
    required_level: SceneLoadLevel,
}

pub(super) struct DependencyDAG {
    nodes: HashMap<DependencyNode, DependencyEntry>,
}

impl DependencyDAG {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    pub fn add_scene(&mut self, scene: &Scene, entity_manager: &EntityManager) {
        let scene_node = DependencyNode::Scene(scene.scene_id);
        if self.nodes.get(&scene_node).is_some() {
            return;
        }
        let mut scene_entry = DependencyEntry {
            children: vec![],
            parents: vec![],
            required_level: scene.load_level,
        };

        for entity in scene.entitites.iter() {
            // if the entity already exists, add this scene as its parent
            if let Some(existing_entity_entry) =
                self.nodes.get_mut(&DependencyNode::Entity(*entity))
            {
                existing_entity_entry.parents.push(scene_node);
                scene_entry.children.push(DependencyNode::Entity(*entity));
            }
            // otherwise, create a new entry with this scene as its parent
            else {
                let mut entity_entry = DependencyEntry {
                    children: vec![],
                    parents: vec![scene_node],
                    required_level: scene.load_level,
                };

                for asset in entity_manager.rbcs_of(*entity) {
                    // if this asset already exists, add this entity as its parent
                    if let Some(existing_asset_entry) =
                        self.nodes.get_mut(&DependencyNode::Asset(asset))
                    {
                        existing_asset_entry
                            .parents
                            .push(DependencyNode::Entity(*entity));
                        entity_entry.children.push(DependencyNode::Asset(asset));
                    } else {
                        let asset_entry = DependencyEntry {
                            children: vec![],
                            parents: vec![DependencyNode::Entity(*entity)],
                            required_level: scene.load_level,
                        };
                        entity_entry.children.push(DependencyNode::Asset(asset));
                        self.nodes.insert(DependencyNode::Asset(asset), asset_entry);
                    }
                }
                self.nodes
                    .insert(DependencyNode::Entity(*entity), entity_entry);
            }
        }

        self.nodes.insert(scene_node, scene_entry);
    }

    pub fn remove_scene(
        &mut self,
        scene_id: SceneId,
    ) -> Result<Vec<DependencyNode>, DependencyDAGError> {
        let scene_node = DependencyNode::Scene(scene_id);
        let scene_entry =
            self.nodes
                .remove(&scene_node)
                .ok_or(DependencyDAGError::NodeNotFound(
                    "cannot find this scene node to remove ".into(),
                ))?;
        for parent in scene_entry.parents.iter() {
            let entry = self
                .nodes
                .get_mut(&parent)
                .ok_or(DependencyDAGError::InvalidParent(
                    "cannot find this parent".into(),
                ))?;
            entry.children.swap_remove(
                entry.children.iter().position(|s| *s == scene_node).ok_or(
                    DependencyDAGError::NodeNotFound(format!(
                        "the removed scene was not present in its parents child list"
                    )),
                )?,
            );
        }
        let mut dead = vec![];
        let mut worklist = vec![(scene_node, scene_entry)];

        while let Some((node, entry)) = worklist.pop() {
            dead.push(node);

            // iterate over each child to remove this node as its parent
            for child in entry.children {
                let child_entry =
                    self.nodes
                        .get_mut(&child)
                        .ok_or(DependencyDAGError::InvalidChild(format!(
                            "{child:?} is in the entries child list but could not be found in graph"
                        )))?;

                let pos = child_entry.parents.iter().position(|p| *p == node).ok_or(
                    DependencyDAGError::InvalidParent(format!(
                        "{node:?} is listed as a parent but could not be found in graph"
                    )),
                )?;

                child_entry.parents.swap_remove(pos);

                // if this child is orphaned, we need to remove it as a parent from all ITS
                // children, so repeat
                if child_entry.parents.is_empty() {
                    let child_entry = self.nodes.remove(&child).unwrap();
                    worklist.push((child, child_entry));
                }
            }
        }

        Ok(dead)
    }
}
