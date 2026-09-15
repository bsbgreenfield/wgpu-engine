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
    jobs: HashMap<AssetJobKey, AssetLoadJob>,
}

impl LoadQueue {
    pub(super) fn add_load_job(
        &mut self,
        update: ((EntityHandle, AssetHandle), SceneLoadLevel),
        asset_manager: &AssetManager,
    ) {
        let (entity, asset) = &update.0;
        let target = update.1;
        let current = asset_manager.res_level_of(asset).expect("can't find asset");
        let key = AssetJobKey {
            entity_handle: *entity,
            asset_handle: *asset,
        };
        if current == target {
            self.jobs.remove(&key);
            return;
        }
        self.jobs
            .entry(key)
            .and_modify(|j| j.target = target)
            .or_insert(AssetLoadJob {
                target,
                base: SceneLoadLevel::from(&current),
            });
    }

    pub(super) fn poll_jobs(
        &mut self,
        asset_manager: &mut AssetManager,
        entity_manager: &EntityManager,
    ) -> Result<Vec<AssetTransition>, AssetLoadError> {
        let mut res: Vec<AssetTransition> = Vec::new();
        let jobs: Vec<(AssetJobKey, AssetLoadJob)> = self
            .jobs
            .iter()
            .map(|(key, asset_job)| (*key, asset_job.clone()))
            .collect();
        for (key, job) in jobs.iter() {
            println!("JOB with base: {:?} and target: {:?}", job.base, job.target);
            let current = asset_manager.res_level_of(&key.asset_handle)?;
            if current == job.target {
                self.jobs
                    .remove_entry(key)
                    .ok_or(AssetLoadError::AssetNotFound)?;
                res.push(AssetTransition {
                    handle: key.asset_handle,
                    old: job.base,
                    new: job.target,
                });
                continue;
            }
            if job.target == SceneLoadLevel::GPU {
                let asset_deps =
                    entity_manager.asset_dependencies_of(&key.entity_handle, &key.asset_handle);

                if !asset_deps.is_empty()
                    && asset_deps.iter().any(|ah| {
                        !matches!(
                            asset_manager.res_level_of(ah).unwrap(),
                            AssetResidency::GPU(_, _)
                        )
                    })
                {
                    continue;
                }
            }
            let after = asset_manager.set_minimum_load_level(&key.asset_handle, job.target)?;
            if after != current {
                res.push(AssetTransition {
                    handle: key.asset_handle,
                    old: SceneLoadLevel::from(&current),
                    new: SceneLoadLevel::from(&after),
                });
                self.jobs.get_mut(&key).unwrap().base = SceneLoadLevel::from(&after);
            }
        }

        Ok(res)
    }
}
