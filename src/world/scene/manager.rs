use std::{error::Error, fmt::Display};

use crate::world::{
    entity_manager::entity_manager::EntityManager,
    instance_manager::archetypes::Archetype,
    scene::{
        SceneEvent, SceneId, SceneLoadLevel, SceneNew, SceneRuntime,
        builder::SceneBuilder,
        dependency_graph::{DependencyGraph, DependencyGraphError},
        scene::Spawn,
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
    dirty_list: Vec<bool>,
    dependency_graph: DependencyGraph,
}

impl SceneManager {
    pub fn new() -> Self {
        SceneManager {
            scenes: vec![],
            dependency_graph: DependencyGraph::new(),
            dirty_list: vec![],
        }
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
            runtime: SceneRuntime::new(),
        };
        self.dependency_graph
            .add_scene(&new_scene, entity_manager)
            .map_err(|de| SceneManagerError::DependencyGraph(de))?;
        self.scenes.push(new_scene);
        self.dirty_list.push(false);
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
            .push(SceneEvent::LoadLevelChanged(
                modified_scene.runtime.current_state,
                level,
            ));
        modified_scene.runtime.requested_level = level;
        self.dependency_graph.set_load_level(scene_id, level)?;
        self.dirty_list[scene_id.0] = true;
        Ok(())
    }

    pub fn add_instances(
        &mut self,
        scene_id: SceneId,
        mut spawn_data: Vec<Spawn<dyn Archetype>>,
    ) -> Result<(), SceneManagerError> {
        let scene = self
            .scenes
            .get_mut(scene_id.0)
            .ok_or(SceneManagerError::SpawnError)?;

        for spawn_datum in spawn_data.drain(..) {
            scene
                .desc
                .entities
                .iter_mut()
                .find(|e| e.0 == spawn_datum.entity)
                .ok_or(SceneManagerError::SpawnError)?
                .1
                .push(spawn_datum.data);
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
        self.dirty_list[id.0]
    }

    pub(super) fn graph(&self) -> &DependencyGraph {
        &self.dependency_graph
    }

    pub(super) fn graph_mut(&mut self) -> &mut DependencyGraph {
        &mut self.dependency_graph
    }
}
