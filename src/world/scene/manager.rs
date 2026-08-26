use std::{error::Error, fmt::Display};

use crate::{
    asset_manager::AssetHandle,
    world::{
        entity_manager::entity_manager::EntityManager,
        instance_manager::{archetypes::Archetype, instance_manager::InstanceManager},
        scene::{
            SceneEvent, SceneId, SceneLoadLevel, SceneNew, SceneRuntime,
            builder::SceneBuilder,
            dependency_graph::{DependencyGraph, DependencyGraphError},
            scene::Spawn,
        },
    },
};

#[derive(Debug)]
pub enum SceneManagerError {
    SpawnError,
    LoadLevelUpdateError,
    DependencyGraph(DependencyGraphError),
}

impl From<DependencyGraphError> for SceneManagerError {
    fn from(value: DependencyGraphError) -> Self {
        Self::DependencyGraph(value)
    }
}
impl Error for SceneManagerError {}
impl Display for SceneManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpawnError => f.write_str("failed to spawn"),
            Self::LoadLevelUpdateError => f.write_str("failed to update load level"),
            Self::DependencyGraph(de) => de.fmt(f),
        }
    }
}

pub struct SceneManager {
    scenes: Vec<SceneNew>,
    dirty_list: Vec<SceneId>,
    dependency_graph: DependencyGraph,
    pub spawn_queue: Vec<Spawn<dyn Archetype>>,
}

impl SceneManager {
    pub fn new() -> Self {
        SceneManager {
            scenes: vec![],
            dependency_graph: DependencyGraph::new(),
            dirty_list: vec![],
            spawn_queue: vec![],
        }
    }

    pub fn asset_updates<'frame>(&'frame mut self) -> Vec<(AssetHandle, SceneLoadLevel)> {
        self.dependency_graph
            .load_results
            .asset_updates
            .drain()
            .collect()
    }

    pub fn process_scene_events(&mut self) {
        for scene_id in self.dirty_list.drain(..) {
            let scene = self.scenes.get_mut(scene_id.0).unwrap();
            if scene.runtime.requested_level == scene.runtime.current_state {
                if !scene.runtime.spawn_queue.is_empty() {
                    self.spawn_queue
                        .extend(std::mem::take(&mut scene.runtime.spawn_queue));
                }
            }
            self.dependency_graph
                .set_load_level(scene_id, scene.runtime.requested_level);
        }
    }

    pub fn get_scene(&self, idx: usize) -> &SceneNew {
        self.scenes.get(idx).expect("scene exists")
    }

    pub fn add_scene(
        &mut self,
        scene: SceneBuilder,
        entity_manager: &EntityManager,
    ) -> Result<SceneId, SceneManagerError> {
        let id = SceneId(self.scenes.len());
        let new_scene = SceneNew {
            id,
            desc: scene.desc,
            runtime: SceneRuntime::default(),
        };
        self.dependency_graph
            .add_scene(&new_scene, entity_manager)
            .map_err(|de| SceneManagerError::DependencyGraph(de))?;
        self.scenes.push(new_scene);
        Ok(id)
    }

    pub fn set_load_level(
        &mut self,
        scene_id: SceneId,
        level: SceneLoadLevel,
    ) -> Result<(), SceneManagerError> {
        let modified_scene = self
            .scenes
            .get_mut(scene_id.0)
            .ok_or(SceneManagerError::LoadLevelUpdateError)?;

        modified_scene
            .runtime
            .event_queue
            .push(SceneEvent::SpawnNew);

        modified_scene
            .runtime
            .event_queue
            .push(SceneEvent::LoadLevelChanged(
                modified_scene.runtime.current_state,
                level,
            ));
        modified_scene.runtime.requested_level = level;
        self.dirty_list.push(modified_scene.id);
        Ok(())
    }

    pub fn add_instances(
        &mut self,
        scene_id: SceneId,
        spawn_data: Vec<Spawn<dyn Archetype>>,
    ) -> Result<(), SceneManagerError> {
        let scene = self
            .scenes
            .get_mut(scene_id.0)
            .ok_or(SceneManagerError::SpawnError)?;
        for spawn in spawn_data {
            scene.runtime.spawn_queue.push(spawn);
        }

        Ok(())
    }
}

#[cfg(test)]
impl SceneManager {
    pub(super) fn scene_count(&self) -> usize {
        self.scenes.len()
    }

    pub(super) fn scene(&self, id: SceneId) -> Option<&SceneNew> {
        self.scenes.get(id.0)
    }

    pub(super) fn is_dirty(&self, id: SceneId) -> bool {
        self.dirty_list.contains(&id)
    }

    pub(super) fn graph(&self) -> &DependencyGraph {
        &self.dependency_graph
    }

    pub(super) fn graph_mut(&mut self) -> &mut DependencyGraph {
        &mut self.dependency_graph
    }
}
