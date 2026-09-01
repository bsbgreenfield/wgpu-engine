use std::collections::HashMap;

use crate::{
    asset_manager::{AssetHandle, AssetLoadError, AssetResidency, asset_manager::AssetManager},
    world::scene::SceneLoadLevel,
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
        let target = update.1;
        let current = asset_manager
            .res_level_of(&update.0)
            .expect("can't find asset");
        if current == target {
            self.jobs.remove(&update.0);
            return;
        }
        self.jobs
            .entry(update.0)
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
        let jobs: Vec<(AssetHandle, AssetLoadJob)> =
            self.jobs.iter().map(|(ah, aj)| (*ah, aj.clone())).collect();
        for (asset_handle, job) in jobs.iter() {
            println!("JOB with base: {:?} and target: {:?}", job.base, job.target);
            let current = asset_manager.res_level_of(asset_handle)?;
            if current == job.target {
                let (handle, _) = self
                    .jobs
                    .remove_entry(asset_handle)
                    .ok_or(AssetLoadError::AssetNotFound)?;
                res.push(AssetTransition {
                    handle: handle,
                    old: job.base,
                    new: job.target,
                });
                continue;
            }
            if matches!(
                current,
                AssetResidency::PendingCPU | AssetResidency::PendingGPU(_)
            ) {
                panic!("HERE");
            }

            let after = asset_manager.set_minimum_load_level(asset_handle, job.target)?;
            println!("BEFORE: {:?}, AFTER: {:?}", current, after);
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
