use std::collections::HashMap;

use crate::{
    asset_manager::{AssetHandle, AssetLoadError, AssetResidency, asset_manager::AssetManager},
    world::scene::SceneLoadLevel,
};

#[derive(Clone)]
struct AssetLoadJob {
    target: SceneLoadLevel,
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
        println!(" ASSET {:?}", update.0);
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
            .or_insert(AssetLoadJob { target });
    }

    pub(super) fn poll_jobs(
        &mut self,
        asset_manager: &mut AssetManager,
    ) -> Result<Vec<AssetTransition>, AssetLoadError> {
        let mut res: Vec<AssetTransition> = Vec::new();
        let jobs: Vec<(AssetHandle, AssetLoadJob)> =
            self.jobs.iter().map(|(ah, aj)| (*ah, aj.clone())).collect();
        for (asset_handle, job) in jobs.iter() {
            let current = asset_manager.res_level_of(asset_handle)?;
            if current == job.target {
                self.jobs.remove(asset_handle);
                continue;
            }
            if matches!(
                current,
                AssetResidency::PendingCPU | AssetResidency::PendingGPU(_)
            ) {
                continue;
            }

            let after = asset_manager.set_minimum_load_level(asset_handle, job.target)?;
            if after != current {
                res.push(AssetTransition {
                    handle: *asset_handle,
                    old: SceneLoadLevel::from(&current),
                    new: SceneLoadLevel::from(&after),
                });
            }
        }

        Ok(res)
    }
}
