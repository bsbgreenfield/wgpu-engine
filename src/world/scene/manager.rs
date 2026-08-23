use std::{error::Error, fmt::Display};

use crate::world::{
    instance_manager::archetypes::Archetype,
    scene::{
        SceneEvent, SceneId, SceneLoadLevel, SceneNew, SceneRuntime, builder::SceneBuilder,
        dependency_graph::DependencyGraph, scene::Spawn,
    },
};

#[derive(Debug)]
pub enum SceneManagerError {
    SpawnError,
    LoadLevelUpdateError,
}
impl Error for SceneManagerError {}
impl Display for SceneManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpawnError => f.write_str("failed to spawn"),
            Self::LoadLevelUpdateError => f.write_str("failed to update load level"),
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

    pub fn add_scene(&mut self, scene: SceneBuilder) -> SceneId {
        let id = SceneId(self.scenes.len());
        let new_scene = SceneNew {
            id,
            desc: scene.desc,
            runtime: SceneRuntime::new(),
        };
        self.scenes.push(new_scene);
        self.dirty_list.push(false);
        // dep graph?
        id
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
