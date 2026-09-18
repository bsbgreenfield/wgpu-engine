use std::collections::HashMap;

use crate::{
    asset_manager::{AssetHandle, AssetLoadError, AssetResidency, asset_manager::AssetManager},
    common::entity::EntityHandle,
    world::{entity_manager::entity_manager::EntityManager, scene::SceneLoadLevel},
};

#[derive(Clone)]
struct AssetLoadJob {
    target: SceneLoadLevel,
    base: SceneLoadLevel,
}

pub struct AssetTransition {
    pub handle: AssetHandle,
    pub old: SceneLoadLevel,
    pub new: SceneLoadLevel,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
struct AssetJobKey {
    entity_handle: EntityHandle,
    asset_handle: AssetHandle,
}

#[derive(Default)]
pub struct LoadQueue {
    jobs: HashMap<AssetHandle, AssetLoadJob>,
}

impl LoadQueue {
    pub(super) fn add_load_job(
        &mut self,
        update: (AssetHandle, SceneLoadLevel),
        asset_manager: &AssetManager,
    ) {
        let asset = &update.0;
        let target = update.1;
        let current = asset_manager.res_level_of(asset).expect("can't find asset");
        if current == target {
            self.jobs.remove(asset);
            return;
        }
        self.jobs
            .entry(*asset)
            .and_modify(|j| j.target = target)
            .or_insert(AssetLoadJob {
                target,
                base: SceneLoadLevel::from(&current),
            });
    }

    pub(super) fn poll_jobs(
        &mut self,
        asset_manager: &mut AssetManager,
    ) -> Result<Vec<AssetTransition>, AssetLoadError> {
        let mut res: Vec<AssetTransition> = Vec::new();
        let jobs: Vec<(AssetHandle, AssetLoadJob)> = self
            .jobs
            .iter()
            .map(|(handle, asset_job)| (*handle, asset_job.clone()))
            .collect();
        for (asset_handle, job) in jobs.iter() {
            println!("JOB with base: {:?} and target: {:?}", job.base, job.target);
            let current = asset_manager.res_level_of(asset_handle)?;
            if current == job.target {
                self.jobs
                    .remove_entry(asset_handle)
                    .ok_or(AssetLoadError::AssetNotFound)?;
                res.push(AssetTransition {
                    handle: *asset_handle,
                    old: job.base,
                    new: job.target,
                });
                continue;
            }
            if job.target == SceneLoadLevel::GPU {
                if !asset_manager.deps_gpu_ready(asset_handle) {
                    continue;
                }
            }
            let after = asset_manager.set_minimum_load_level(asset_handle, job.target)?;
            if after != current {
                res.push(AssetTransition {
                    handle: *asset_handle,
                    old: SceneLoadLevel::from(&current),
                    new: SceneLoadLevel::from(&after),
                });
                self.jobs.get_mut(asset_handle).unwrap().base = SceneLoadLevel::from(&after);
            }
        }

        Ok(res)
    }
}
