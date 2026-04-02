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
        Ok(deltas)
    }

    pub(super) fn poll_entity_jobs(&mut self) {
        if self.entity_jobs.len() == 0 {
            return;
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
        // .map(|(entity_handle, entity_job)| {
        //     let allocations = entity_job
        //         .asset_load_jobs
        //         .into_iter()
        //         .map(|asset_handle| {
        //             let alloc = match &self.asset_jobs.get(&asset_handle).unwrap().state {
        //                 AssetLoadJobState::Done(load_result) => match load_result {
        //                     AssetLoadResult::LoadedGPU(alloc_handle) => alloc_handle.clone(),
        //                     _ => todo!(),
        //                 },
        //                 _ => unreachable!(),
        //             };

        //             (asset_handle, alloc)
        //         })
        //         .collect();
        //     (entity_handle, allocations)
        // })
        // .collect();
        self.completed_queue.extend(completed_entities);
    }
}
