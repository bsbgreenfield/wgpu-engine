use std::collections::{HashMap, hash_map::Entry};

use crate::{
    asset_manager::{AssetHandle, AssetLoadError, AssetResidency, asset_manager::AssetManager},
    world::scene::SceneLoadLevel,
};

#[derive(Clone)]
struct AssetLoadJob {
    target: SceneLoadLevel,
}

#[derive(Default)]
pub struct LoadQueueNew {
    jobs: HashMap<AssetHandle, AssetLoadJob>,
    pub pending_gpu: Vec<AssetHandle>,
}

impl LoadQueueNew {
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
        } else if current < target {
            if let Some(job) = self.jobs.get_mut(&update.0) {
                job.target = target;
            } else {
                self.jobs.insert(update.0, AssetLoadJob { target });
            }
        } else {
            todo!("downgrade job")
        }
    }

    pub(super) fn poll_jobs(
        &mut self,
        asset_manager: &mut AssetManager,
    ) -> Result<(), AssetLoadError> {
        let jobs: Vec<(AssetHandle, AssetLoadJob)> =
            self.jobs.iter().map(|(ah, aj)| (*ah, aj.clone())).collect();
        for (asset_handle, job) in jobs.iter() {
            let current = asset_manager.res_level_of(asset_handle)?;
            if current >= job.target {
                self.jobs.remove(asset_handle);
                continue;
            }
            if matches!(
                current,
                AssetResidency::PendingCPU | AssetResidency::PendingGPU(_)
            ) {
                continue;
            }
            if matches!(
                asset_manager.set_minumum_load_level(asset_handle, job.target)?,
                AssetResidency::PendingGPU(_)
            ) {
                self.pending_gpu.push(*asset_handle);
            }
        }

        Ok(())
    }
}
