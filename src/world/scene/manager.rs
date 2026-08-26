use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Display,
};

use crate::{
    asset_manager::{AssetHandle, asset_manager::AssetManager},
    common::instance::InstanceHandle,
    world::{
        entity_manager::entity_manager::EntityManager,
        instance_manager::archetypes::Archetype,
        scene::{
            SceneEvent, SceneId, SceneLoadLevel, SceneNew, SceneRuntime, builder::SceneBuilder,
            dep_graph_2::DependencyGraphNew, dependency_graph::DependencyGraphError, scene::Spawn,
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
    dependency_graph: DependencyGraphNew,
    pub spawn_queue: Vec<Spawn<dyn Archetype>>,
    asset_requests: HashMap<AssetHandle, SceneLoadLevel>,
    visited: HashMap<SceneId, SceneLoadLevel>,
}

impl SceneManager {
    pub fn new() -> Self {
        SceneManager {
            scenes: vec![],
            dependency_graph: DependencyGraphNew::default(),
            spawn_queue: vec![],
            asset_requests: HashMap::new(),
            visited: HashMap::new(),
        }
    }

    pub fn asset_requests<'frame>(&'frame mut self) -> Vec<(AssetHandle, SceneLoadLevel)> {
        self.asset_requests.drain().collect()
    }

    pub fn process_scene_events(
        &mut self,
        asset_manager: &AssetManager,
    ) -> Result<(), SceneManagerError> {
        let mut ready: Vec<SceneId> = Vec::new();
        for idx in 0..self.scenes.len() {
            let (scene_id, requested) = {
                let scene = &self.scenes[idx];
                if !scene.runtime.needs_update() {
                    continue;
                }
                (scene.id, scene.runtime.requested_level)
            };
            self.visited.clear();
            let reached = self.dependency_graph.resolve(
                scene_id,
                requested,
                asset_manager,
                &mut self.asset_requests,
                &mut self.visited,
            )?;

            let actual = reached.min(requested);
            let scene = &mut self.scenes[idx];

            if actual != scene.runtime.current_state {
                scene.runtime.event_queue.push(SceneEvent::LoadLevelChanged(
                    scene.runtime.current_state,
                    actual,
                ));
            }
            if scene.runtime.ready_to_spawn() {
                ready.push(scene_id);
            }
        }

        for scene_id in ready {
            self.do_spawns(scene_id);
        }
        Ok(())
    }

    fn do_spawns(&mut self, root: SceneId) {
        let mut stack = vec![root];
        let mut seen = HashSet::new();
        while let Some(scene_id) = stack.pop() {
            if !seen.insert(scene_id) {
                continue;
            }
            let spawns = self
                .scenes
                .get_mut(scene_id.0)
                .map(|s| std::mem::take(&mut s.runtime.spawn_queue))
                .unwrap_or_default();
            self.spawn_queue.extend(spawns);
            stack.extend_from_slice(self.dependency_graph.children_of(scene_id));
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
            .push(SceneEvent::LoadLevelChanged(
                modified_scene.runtime.current_state,
                level,
            ));
        modified_scene.runtime.requested_level = level;
        Ok(())
    }

    pub fn add_instances(
        &mut self,
        scene_id: SceneId,
        spawn_data: Vec<Spawn<dyn Archetype>>,
    ) -> Result<(), SceneManagerError> {
        self.scenes
            .get_mut(scene_id.0)
            .ok_or(SceneManagerError::SpawnError)?
            .runtime
            .spawn_queue
            .extend(spawn_data);

        Ok(())
    }

    pub fn add_instance_handles(
        &mut self,
        scene_id: SceneId,
        handles: impl IntoIterator<Item = InstanceHandle>,
    ) -> Result<(), SceneManagerError> {
        let scene = self
            .scenes
            .get_mut(scene_id.0)
            .ok_or(SceneManagerError::SpawnError)?;
        scene.runtime.instances.extend(handles);

        Ok(())
    }
}
