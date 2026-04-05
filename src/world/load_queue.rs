use std::collections::{HashMap, HashSet};

use crate::{
    asset_manager::{
        AssetHandle,
        asset_manager::{AssetLoadResult, AssetManager},
    },
    world::{
        WorldUpdateError, entity_manager::EntityHandle, scene::SceneLoadLevel,
        world::WorldUpdateDelta,
    },
};

pub(super) struct EntityLoadJob {
    pub(super) load_level: SceneLoadLevel,
    pub(super) asset_load_jobs: Vec<AssetHandle>,
}

#[derive(Debug)]
enum AssetLoadJobState {
    Done,
    Pending,
}
struct AssetLoadJob {
    state: AssetLoadJobState,
    ref_count: usize,
}
pub(super) struct EntityLoadQueue {
    pub(super) completed_queue: HashMap<EntityHandle, EntityLoadJob>,
    entity_jobs: HashMap<EntityHandle, EntityLoadJob>,
    asset_jobs: HashMap<AssetHandle, AssetLoadJob>,
}

impl EntityLoadQueue {
    pub(super) fn new() -> Self {
        Self {
            completed_queue: HashMap::new(),
            entity_jobs: HashMap::new(),
            asset_jobs: HashMap::new(),
        }
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

    pub(super) fn new_entity_load(
        &mut self,
        entity: EntityHandle,
        load_level: SceneLoadLevel,
        assets: &HashSet<AssetHandle>,
    ) -> Result<&EntityLoadJob, WorldUpdateError> {
        let s = self.entity_jobs.insert(
            entity,
            EntityLoadJob {
                load_level,
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
                        state: AssetLoadJobState::Pending,
                    },
                );
            } else {
                self.asset_jobs.get_mut(asset).unwrap().ref_count += 1;
            }
        }
        Ok(self.entity_jobs.get(&entity).as_ref().unwrap())
    }

    pub(super) fn poll_assets_for_job(
        &mut self,
        entity: EntityHandle,
        asset_manager: &mut AssetManager,
    ) -> Result<Vec<WorldUpdateDelta>, WorldUpdateError> {
        let job = self.entity_jobs.get(&entity).unwrap();
        let mut deltas = Vec::new();
        for asset in job.asset_load_jobs.iter() {
            match self.asset_jobs.get_mut(asset).unwrap().state {
                AssetLoadJobState::Done => continue,
                AssetLoadJobState::Pending => {
                    let asset_load_result = asset_manager
                        .set_minumum_load_level(*asset, job.load_level)
                        .unwrap();

                    if asset_load_result.is_greater_than_or_equal_to(job.load_level) {
                        let asset_job = self.asset_jobs.get_mut(asset).unwrap();
                        asset_job.state = AssetLoadJobState::Done;
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
        Ok(deltas)
    }

    pub(super) fn poll_entity_jobs(
        &mut self,
        manager: &mut AssetManager,
    ) -> Result<Option<Vec<WorldUpdateDelta>>, WorldUpdateError> {
        if self.entity_jobs.len() == 0 {
            return Ok(None);
        }
        let mut res = Vec::new();
        let handle_iter: Vec<EntityHandle> = self
            .entity_jobs
            .iter()
            .map(|entry| entry.0.clone())
            .collect();

        for entity_handle in handle_iter {
            res.extend(self.poll_assets_for_job(entity_handle, manager)?);
        }
        let completed_entities: HashMap<EntityHandle, EntityLoadJob> = self
            .entity_jobs
            .extract_if(|_, entity_job| {
                entity_job.asset_load_jobs.iter().all(|asset_handle| {
                    matches!(
                        self.asset_jobs.get(asset_handle).unwrap().state,
                        AssetLoadJobState::Done
                    )
                })
            })
            .collect();
        self.completed_queue.extend(completed_entities);
        Ok(Some(res))
    }
}
