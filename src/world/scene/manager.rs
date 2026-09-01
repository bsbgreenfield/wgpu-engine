use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Display,
};

use crate::{
    asset_manager::{AssetHandle, asset_manager::AssetManager},
    common::instance::InstanceHandle,
    renderer::GPUInstanceHandle,
    world::{
        entity_manager::entity_manager::EntityManager,
        instance_manager::archetypes::Archetype,
        load_queue::{AssetTransition, LoadQueue},
        scene::{
            Scene, SceneId, SceneLoadLevel, SceneRuntime,
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
    scenes: Vec<Scene>,
    dependency_graph: DependencyGraph,
    pub spawn_queue: HashMap<SceneId, Vec<Spawn<dyn Archetype>>>,
    pub despawn_queue: Vec<InstanceHandle>,
    asset_requests: HashMap<AssetHandle, SceneLoadLevel>,
    pending: Vec<usize>,
    ready: Vec<SceneId>,
    pub load_queue_new: LoadQueue,
    pub asset_release_queue: HashSet<AssetHandle>,
    pub inflight_despawns: HashMap<GPUInstanceHandle, InstanceHandle>,
}

impl SceneManager {
    pub fn new() -> Self {
        SceneManager {
            scenes: vec![],
            dependency_graph: DependencyGraph::default(),
            spawn_queue: HashMap::new(),
            despawn_queue: vec![],
            asset_requests: HashMap::new(),
            pending: Vec::new(),
            ready: Vec::new(),
            asset_release_queue: HashSet::new(),
            load_queue_new: LoadQueue::default(),
            inflight_despawns: HashMap::new(),
        }
    }

    pub fn ack_despawns(&mut self, gpu_handles: Vec<GPUInstanceHandle>) {
        for gpu_handle in gpu_handles {
            let Some(instance) = self.inflight_despawns.remove(&gpu_handle) else {
                continue;
            };

            for free_asset in self.dependency_graph.ack_despawn(instance) {
                self.asset_release_queue.remove(&free_asset);
                self.asset_requests.insert(
                    free_asset,
                    self.dependency_graph.required_asset_level(&free_asset),
                );
            }
        }
    }

    pub fn asset_requests<'frame>(&'frame mut self) -> Vec<(AssetHandle, SceneLoadLevel)> {
        self.asset_requests.drain().collect()
    }
    pub fn process_scene_events(&mut self) -> Result<(), SceneManagerError> {
        let Self {
            scenes,
            spawn_queue,
            ready,
            ..
        } = self;
        for scene_id in ready.drain(..) {
            let runtime = &mut scenes[scene_id.0].runtime;
            if runtime.ready_to_spawn() {
                spawn_queue.entry(scene_id).or_insert(vec![]);
                spawn_queue
                    .get_mut(&scene_id)
                    .unwrap()
                    .extend(std::mem::take(&mut runtime.spawn_queue));
            }
        }
        Ok(())
    }

    pub fn get_scene(&self, idx: usize) -> &Scene {
        self.scenes.get(idx).expect("scene exists")
    }

    pub fn add_scene(
        &mut self,
        scene: SceneBuilder,
        entity_manager: &EntityManager,
    ) -> Result<SceneId, SceneManagerError> {
        let id = SceneId(self.scenes.len());
        let new_scene = Scene {
            id,
            desc: scene.desc,
            runtime: SceneRuntime::default(),
        };
        self.dependency_graph
            .add_scene(&new_scene, entity_manager)
            .map_err(|de| SceneManagerError::DependencyGraph(de))?;
        self.scenes.push(new_scene);
        self.pending.push(0);
        Ok(id)
    }
    pub fn set_load_level(
        &mut self,
        scene_id: SceneId,
        level: SceneLoadLevel,
        asset_manager: &AssetManager,
    ) -> Result<(), SceneManagerError> {
        let Self {
            scenes,
            dependency_graph,
            asset_requests,
            despawn_queue,
            pending,
            ready,
            ..
        } = self;

        let scene = scenes
            .get_mut(scene_id.0)
            .ok_or(SceneManagerError::LoadLevelUpdateError)?;
        let previous = scene.runtime.requested_level;
        if previous == level {
            return Ok(());
        }
        scene.runtime.requested_level = level;

        let assets = dependency_graph.recompute_asset_levels(scene_id, previous, level);
        if level > previous {
            // raising one holder can only raise each asset's max, so no other
            // scene's request needs consulting
            let mut count = 0;
            for asset in assets {
                let residency = asset_manager
                    .res_level_of(&asset)
                    .map_err(|_| SceneManagerError::LoadLevelUpdateError)?;
                if SceneLoadLevel::from(&residency) < level {
                    asset_requests.insert(asset, level);
                    count += 1;
                }
            }
            pending[scene_id.0] = count;
            if count == 0 {
                // everything already resident — nothing to wait for
                scenes[scene_id.0].runtime.current_state = level;
                ready.push(scene_id);
            }
        } else {
            // lowering: an asset only drops if every other holder wants less too
            for asset in assets {
                let required = dependency_graph.required_asset_level(&asset);

                let residency = asset_manager
                    .res_level_of(&asset)
                    .map_err(|_| SceneManagerError::LoadLevelUpdateError)?;
                // if the asset level drops from this action, then add to requested
                if SceneLoadLevel::from(&residency) > required {
                    self.asset_release_queue.insert(asset);
                }
            }
            pending[scene_id.0] = 0;

            let runtime = &mut scenes[scene_id.0].runtime;
            let previous_state = runtime.current_state;
            runtime.current_state = level;
            if previous_state == SceneLoadLevel::GPU && level < SceneLoadLevel::GPU {
                despawn_queue.extend(dependency_graph.drain_instances_of(scene_id));
            }
        }
        Ok(())
    }
    pub fn on_asset_level_changed(&mut self, transition: AssetTransition) {
        let Self {
            scenes,
            dependency_graph,
            pending,
            ready,
            ..
        } = self;
        for holder in dependency_graph.holders_of(&transition.handle) {
            let requested = scenes[holder.0].runtime.requested_level;
            // was blocking this holder, no longer is
            if transition.old < requested && transition.new >= requested {
                pending[holder.0] -= 1;
                if pending[holder.0] == 0 {
                    scenes[holder.0].runtime.current_state = requested;
                    ready.push(*holder);
                }
            }
        }
    }

    pub fn add_instances(
        &mut self,
        scene_id: SceneId,
        spawn_data: Vec<Spawn<dyn Archetype>>,
    ) -> Result<(), SceneManagerError> {
        let already_loaded = {
            let scene = self
                .scenes
                .get_mut(scene_id.0)
                .ok_or(SceneManagerError::SpawnError)?;
            scene.runtime.spawn_queue.extend(spawn_data);
            scene.runtime.ready_to_spawn()
        };
        if already_loaded {
            self.ready.push(scene_id);
        }
        Ok(())
    }

    pub fn add_instance_handles(
        &mut self,
        scene_id: SceneId,
        handles: impl IntoIterator<Item = InstanceHandle>,
    ) -> Result<(), SceneManagerError> {
        self.dependency_graph
            .add_instance_handles(scene_id, handles);

        Ok(())
    }

    #[cfg(test)]
    pub fn instances_of(&self, scene_id: SceneId) -> Vec<InstanceHandle> {
        self.dependency_graph
            .clone_instances_of(scene_id)
            .into_iter()
            .collect()
    }
}
