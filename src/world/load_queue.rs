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
    state: LoadJobState,
    pub(super) asset_load_jobs: Vec<AssetHandle>,
}

#[derive(Debug)]
enum LoadJobState {
    Done,
    Pending,
}
struct AssetLoadJob {
    state: LoadJobState,
    ref_count: usize,
}
pub(super) struct EntityLoadQueue {
    pub(super) completed_queue: HashMap<SceneId, SceneLoadJob>,
    scene_jobs: HashMap<SceneId, SceneLoadJob>,
    entity_jobs: HashMap<EntityHandle, EntityLoadJob>,
    asset_jobs: HashMap<AssetHandle, AssetLoadJob>,
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

    pub(super) fn dequeue_completed(&mut self) {
        for (_, job) in self.completed_queue.drain() {
            for asset in job.asset_load_jobs {
                let asset_job = self.asset_jobs.get_mut(&asset).unwrap();
                asset_job.ref_count -= 1;
                if asset_job.ref_count == 0 {
                    self.asset_jobs.remove(&asset);
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
            self.new_entity_load(entity.clone(), &entity_manager.rbcs_of(*entity))?;
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

    pub(super) fn new_entity_load(
        &mut self,
        entity: EntityHandle,
        assets: &HashSet<AssetHandle>,
    ) -> Result<&EntityLoadJob, WorldUpdateError> {
        let s = self.entity_jobs.insert(
            entity,
            EntityLoadJob {
                state: LoadJobState::Pending,
                asset_load_jobs: assets.iter().map(|a| a.to_owned()).collect(),
            },
        );

        if s.is_some() {
            return Err(WorldUpdateError::EntityLoadAlreadyEnqeued(entity));
        }

        for asset in assets {
            if self.asset_jobs.get(asset).is_none() {
                self.asset_jobs.insert(
                    *asset,
                    AssetLoadJob {
                        ref_count: 1,
                        state: LoadJobState::Pending,
                    },
                );
            } else {
                self.asset_jobs.get_mut(asset).unwrap().ref_count += 1;
            }
        }
        Ok(self.entity_jobs.get(&entity).as_ref().unwrap())
    }

    pub fn poll_scene_job(
        &mut self,
        scene_id: SceneId,
        manager: &mut AssetManager,
    ) -> Result<Vec<WorldUpdateDelta>, WorldUpdateError> {
        let mut deltas = Vec::<WorldUpdateDelta>::new();
        let mut complete = true;
        let load_level = self.scene_jobs[&scene_id].load_level;
        let entity_count = self.scene_jobs[&scene_id].entity_load_jobs.len();
        for i in 0..entity_count {
            let entity = self.scene_jobs[&scene_id].entity_load_jobs[i];
            if !self.poll_entity(entity, manager, load_level, &mut deltas)? {
                complete = false;
            }
        }
        if complete {
            let completed_scene_job = self.scene_jobs.remove_entry(&scene_id).unwrap();
            self.completed_queue
                .insert(completed_scene_job.0, completed_scene_job.1);
        }
        Ok(deltas)
    }

    // pub(super) fn poll_scene_jobs(
    //     &mut self,
    //     manager: &mut AssetManager,
    // ) -> Result<(), WorldUpdateError> {
    //     let mut deltas = Vec::<WorldUpdateDelta>::new();
    //     let scenes: Vec<SceneId> = self.scene_jobs.keys().cloned().collect();
    //     for scene in scenes {
    //         let mut complete = true;
    //         let load_level = self.scene_jobs[&scene].load_level;
    //         let entity_count = self.scene_jobs[&scene].entity_load_jobs.len();
    //         for i in 0..entity_count {
    //             let entity = self.scene_jobs[&scene].entity_load_jobs[i];
    //             if !self.poll_entity(entity, manager, load_level, &mut deltas)? {
    //                 complete = false;
    //             }
    //         }
    //         if complete {
    //             let completed_scene_job = self.scene_jobs.remove_entry(&scene).unwrap();
    //             self.completed_queue
    //                 .insert(completed_scene_job.0, completed_scene_job.1);
    //         }
    //     }

    //     todo!()
    // }

    fn poll_entity(
        &mut self,
        entity_handle: EntityHandle,
        manager: &mut AssetManager,
        load_level: SceneLoadLevel,
        deltas: &mut Vec<WorldUpdateDelta>,
    ) -> Result<bool, WorldUpdateError> {
        if self.poll_assets_for_job(entity_handle, manager, load_level, deltas)? {
            return Ok(true);
        } else {
            return Ok(false);
        }
    }

    pub(super) fn poll_assets_for_job(
        &mut self,
        entity: EntityHandle,
        asset_manager: &mut AssetManager,
        load_level: SceneLoadLevel,
        deltas: &mut Vec<WorldUpdateDelta>,
    ) -> Result<bool, WorldUpdateError> {
        let job = self.entity_jobs.get(&entity).unwrap();
        let mut counter = job.asset_load_jobs.len();
        for asset in job.asset_load_jobs.iter() {
            match self.asset_jobs.get_mut(asset).unwrap().state {
                LoadJobState::Done => {
                    counter -= 1;
                    continue;
                }
                LoadJobState::Pending => {
                    let asset_load_result = asset_manager
                        .set_minumum_load_level(*asset, load_level)
                        .unwrap();

                    if asset_load_result.is_greater_than_or_equal_to(load_level) {
                        let asset_job = self.asset_jobs.get_mut(asset).unwrap();
                        asset_job.state = LoadJobState::Done;
                        counter -= 1;
                    } else {
                        match asset_load_result {
                            AssetLoadResult::PendingGPU => {
                                deltas.push(WorldUpdateDelta::AssetDidLoad(*asset));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(counter == 0)
    }
}
