use std::collections::{HashMap, HashSet};

use crate::{
    asset_manager::{
        AssetHandle,
        asset_manager::{AssetLoadResult, AssetManager},
    },
    world::{
        WorldUpdateError,
        entity_manager::{EntityHandle, EntityManager},
        scene::{Scene, SceneId, SceneLoadLevel},
        world::WorldUpdateDelta,
    },
};

pub(super) struct SceneLoadJob {
    pub(super) load_level: SceneLoadLevel,
    pub(super) entity_load_jobs: Vec<EntityHandle>,
}

pub(super) struct EntityLoadJob {
    pub(super) ref_count: usize,
    pub(super) asset_load_jobs: Vec<AssetHandle>,
}

pub(super) struct AssetLoadJob {
    pub(super) current_load_level: SceneLoadLevel,
    pub(super) max_load_level: SceneLoadLevel,
    pub(super) ref_count: usize,
}
pub struct EntityLoadQueue {
    pub(super) completed_queue: HashMap<SceneId, SceneLoadJob>,
    pub(super) scene_jobs: HashMap<SceneId, SceneLoadJob>,
    pub(super) entity_jobs: HashMap<EntityHandle, EntityLoadJob>,
    pub(super) asset_jobs: HashMap<AssetHandle, AssetLoadJob>,
}

impl EntityLoadQueue {
    pub(super) fn new() -> Self {
        Self {
            scene_jobs: HashMap::new(),
            completed_queue: HashMap::new(),
            entity_jobs: HashMap::new(),
            asset_jobs: HashMap::new(),
        }
    }

    pub(super) fn has_pending_scene_job(&self, scene_id: SceneId) -> bool {
        self.scene_jobs.get(&scene_id).is_some()
    }

    pub(super) fn dequeue_spawned_scene(&mut self, scene_id: SceneId) {
        let Some(scene_job) = self.completed_queue.remove(&scene_id) else {
            return;
        };

        for entity in scene_job.entity_load_jobs {
            let entity_ref_count = {
                let job = self.entity_jobs.get_mut(&entity).unwrap();
                job.ref_count -= 1;
                job.ref_count
            };

            if entity_ref_count == 0 {
                let entity_job = self.entity_jobs.remove(&entity).unwrap();
                for asset in entity_job.asset_load_jobs {
                    let asset_job = self.asset_jobs.get_mut(&asset).unwrap();
                    asset_job.ref_count -= 1;
                    if asset_job.ref_count == 0 {
                        self.asset_jobs.remove(&asset);
                    }
                }
            }
        }
    }

    pub(super) fn new_scene_job(
        &mut self,
        scene: &Scene,
        entity_manager: &EntityManager,
    ) -> Result<(), WorldUpdateError> {
        let entities = scene.entitites.clone();
        for entity in entities.iter() {
            self.new_entity_load(
                entity.clone(),
                &entity_manager.rbcs_of(*entity),
                scene.load_level,
            )?;
        }
        self.scene_jobs.insert(
            scene.scene_id,
            SceneLoadJob {
                load_level: scene.load_level,
                entity_load_jobs: entities,
            },
        );

        Ok(())
    }

    fn new_entity_load(
        &mut self,
        entity: EntityHandle,
        assets: &HashSet<AssetHandle>,
        load_level: SceneLoadLevel,
    ) -> Result<&EntityLoadJob, WorldUpdateError> {
        if let Some(existing) = self.entity_jobs.get_mut(&entity) {
            existing.ref_count += 1;
            return Ok(self.entity_jobs.get(&entity).unwrap());
        }
        let s = self.entity_jobs.insert(
            entity,
            EntityLoadJob {
                ref_count: 1,
                asset_load_jobs: assets.iter().map(|a| a.to_owned()).collect(),
            },
        );

        if s.is_some() {
            return Err(WorldUpdateError::EntityLoadAlreadyEnqeued(entity));
        }

        for asset in assets {
            if let Some(asset_job) = self.asset_jobs.get_mut(asset) {
                if load_level > asset_job.max_load_level {
                    asset_job.max_load_level = load_level;
                }
                asset_job.ref_count += 1;
            } else {
                self.asset_jobs.insert(
                    *asset,
                    AssetLoadJob {
                        ref_count: 1,
                        max_load_level: load_level,
                        current_load_level: SceneLoadLevel::NotLoaded,
                    },
                );
            }
        }
        Ok(self.entity_jobs.get(&entity).as_ref().unwrap())
    }

    pub fn poll_scene_job(
        &mut self,
        scene_id: SceneId,
        manager: &mut AssetManager,
        deltas: &mut Vec<WorldUpdateDelta>,
    ) -> Result<(), WorldUpdateError> {
        let mut complete = true;
        let entity_count = self.scene_jobs[&scene_id].entity_load_jobs.len();
        for i in 0..entity_count {
            let load_level = self.scene_jobs[&scene_id].load_level;
            let entity = self.scene_jobs[&scene_id].entity_load_jobs[i];
            if !self.poll_entity(entity, manager, deltas, load_level)? {
                complete = false;
            }
        }
        if complete {
            let completed_scene_job = self.scene_jobs.remove_entry(&scene_id).unwrap();
            self.completed_queue
                .insert(completed_scene_job.0, completed_scene_job.1);
        }
        Ok(())
    }

    fn poll_entity(
        &mut self,
        entity_handle: EntityHandle,
        manager: &mut AssetManager,
        deltas: &mut Vec<WorldUpdateDelta>,
        load_level: SceneLoadLevel,
    ) -> Result<bool, WorldUpdateError> {
        if self.poll_assets_for_job(entity_handle, manager, deltas, load_level)? {
            return Ok(true);
        } else {
            return Ok(false);
        }
    }

    fn poll_assets_for_job(
        &mut self,
        entity: EntityHandle,
        asset_manager: &mut AssetManager,
        deltas: &mut Vec<WorldUpdateDelta>,
        load_level: SceneLoadLevel,
    ) -> Result<bool, WorldUpdateError> {
        let job = self.entity_jobs.get(&entity).unwrap();
        let mut counter = job.asset_load_jobs.len();
        for asset in job.asset_load_jobs.iter() {
            let asset_job = self.asset_jobs.get_mut(asset).unwrap();
            if asset_job.current_load_level >= asset_job.max_load_level {
                counter -= 1;
                continue;
            } else {
                let load_result = asset_manager
                    .set_minumum_load_level(*asset, asset_job.max_load_level)
                    .unwrap();
                if SceneLoadLevel::from(&load_result) >= load_level {
                    counter -= 1;
                } else {
                    match load_result {
                        AssetLoadResult::PendingGPU => {
                            deltas.push(WorldUpdateDelta::AssetDidLoad(*asset));
                        }
                        _ => {}
                    }
                }
                asset_job.current_load_level = SceneLoadLevel::from(&load_result);
            }
        }
        Ok(counter == 0)
    }
}
