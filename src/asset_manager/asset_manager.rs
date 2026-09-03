use std::{collections::HashMap, fmt::Debug, marker::PhantomData};

use crate::{
    app::GPUAssetUploadJob,
    asset_manager::{
        Asset, AssetHandle, AssetLoadError, AssetResidency, LoadedAsset, UnloadedAssetData,
    },
    renderer::GPUAllocationHandle,
    world::{entity_manager::components::ResourceBacking, scene::SceneLoadLevel},
};

pub(super) enum RegisteredAsset<A: Asset + ?Sized> {
    Unloaded {
        data: UnloadedAssetData,
        _t: PhantomData<A>,
    },
    Loaded {
        residency: AssetResidency,
        data: UnloadedAssetData,
        _t: PhantomData<A>,
    },
}

impl<A: Asset + ?Sized> Debug for RegisteredAsset<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unloaded { data, _t } => f
                .debug_struct("Unloaded")
                .field("data", data)
                .field("_t", _t)
                .finish(),
            Self::Loaded { residency, .. } => f.debug_tuple("Loaded").field(residency).finish(),
        }
    }
}

impl<A: Asset + ?Sized> RegisteredAsset<A> {
    fn set_as_gpu_loaded(
        &mut self,
        alloc_handle: GPUAllocationHandle,
    ) -> Result<(), AssetLoadError> {
        let Self::Loaded { residency: res, .. } = self else {
            return Err(AssetLoadError::AssetNotLoaded(
                "tried to set as gpu loaded an asset that was not yet cpu resident".into(),
            ));
        };
        if let AssetResidency::PendingGPU(idx) = res {
            *res = AssetResidency::GPU(alloc_handle, *idx);
            return Ok(());
        } else {
            return Err(AssetLoadError::AssetNotLoaded(format!(
                "tried to set gpu loaded on asset with residency of {res:?}"
            )));
        }
    }
}

pub struct AssetManager {
    registered_assets: HashMap<AssetHandle, RegisteredAsset<dyn Asset>>,
    loaded_assets: Vec<(AssetHandle, Box<dyn Asset>)>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            loaded_assets: Vec::new(),
            registered_assets: HashMap::new(),
        }
    }
    fn gen_handle(&self) -> AssetHandle {
        AssetHandle(self.registered_assets.len() as u32)
    }

    pub(crate) fn res_level_of(
        &self,
        asset_handle: &AssetHandle,
    ) -> Result<AssetResidency, AssetLoadError> {
        //println!("HERE!!!! {:?}", self.registered_assets);
        let registered = self
            .registered_assets
            .get(asset_handle)
            .ok_or(AssetLoadError::AssetNotFound)?;
        match registered {
            RegisteredAsset::Unloaded { data: _data, _t } => Ok(AssetResidency::Registered),
            RegisteredAsset::Loaded { residency: res, .. } => Ok(res.clone()),
        }
    }

    fn unload(&mut self, idx: usize) -> Result<(), AssetLoadError> {
        let (handle, _unloaded) = self.loaded_assets.swap_remove(idx);
        let entry = self
            .registered_assets
            .remove(&handle)
            .expect("asset was registered");
        match entry {
            RegisteredAsset::Loaded {
                residency: _,
                data,
                _t,
            } => {
                self.registered_assets
                    .insert(handle, RegisteredAsset::Unloaded { data, _t });
            }
            _ => panic!("not loaded"),
        }

        //        self.registered_assets.entry(*handle).and_modify(|ra| *ra = RegisteredAsset::Unloaded { data: , _t: () })
        if self.loaded_assets.len() > 0 {
            let last = &self.loaded_assets.last().as_ref().unwrap().0;
            match self
                .registered_assets
                .get_mut(last)
                .expect("should be registered")
            {
                RegisteredAsset::Unloaded { .. } => {}
                RegisteredAsset::Loaded { residency: res, .. } => {
                    res.update_la_idx(idx);
                }
            }
        }
        Ok(())
    }
    fn load(&mut self, asset_handle: &AssetHandle) -> Result<usize, AssetLoadError> {
        let registered_asset = self.registered_assets.remove(asset_handle).unwrap();
        match registered_asset {
            RegisteredAsset::Unloaded { data, _t } => {
                let loaded = data.load()?;
                let la_index = self.loaded_assets.len().clone();
                self.loaded_assets.push((asset_handle.clone(), loaded));
                self.registered_assets.insert(
                    *asset_handle,
                    RegisteredAsset::Loaded {
                        residency: AssetResidency::CPU(la_index),
                        data,
                        _t,
                    },
                );
                return Ok(la_index);
            }
            RegisteredAsset::Loaded { residency: res, .. } => match res {
                AssetResidency::CPU(la_index) => return Ok(la_index),
                AssetResidency::GPU(_alloc, la_index) => return Ok(la_index),
                _ => panic!(),
            },
        }
    }

    pub fn get_upload_job_for<'a>(
        &'a self,
        asset_handle: AssetHandle,
    ) -> Result<GPUAssetUploadJob, AssetLoadError> {
        match self.registered_assets.get(&asset_handle).unwrap() {
            RegisteredAsset::Unloaded { data: _data, _t } => Err(AssetLoadError::AssetNotLoaded(
                String::from("this asset is not yet loaded!"),
            )),
            RegisteredAsset::Loaded { residency: res, .. } => match res {
                AssetResidency::CPU(la_index) | AssetResidency::PendingGPU(la_index) => {
                    println!("this asset is {:?} RES", res);
                    let asset = &self.loaded_assets[*la_index].1;
                    return asset.get_upload_job(asset_handle);
                }
                _ => return Err(AssetLoadError::AssetNotFound),
            },
        }
    }

    pub(crate) fn alloc_handle_of(
        &self,
        asset_handle: &AssetHandle,
    ) -> Result<GPUAllocationHandle, AssetLoadError> {
        match self
            .registered_assets
            .get(asset_handle)
            .ok_or(AssetLoadError::AssetNotFound)?
        {
            RegisteredAsset::Loaded {
                residency,
                data: _,
                _t,
            } => match residency {
                AssetResidency::GPU(alloc_handle, _)
                | AssetResidency::PendingUnloadGPU(alloc_handle, _) => {
                    return Ok(alloc_handle.clone());
                }
                _ => {
                    return Err(AssetLoadError::AssetNotLoaded(
                        "Asset is not GPU Loaded".to_string(),
                    ));
                }
            },
            RegisteredAsset::Unloaded { data: _, _t } => {
                todo!()
            }
        }
    }
    pub fn register_asset<A>(&mut self, source: &str) -> Result<ResourceBacking<A>, AssetLoadError>
    where
        A: Asset + 'static,
    {
        let asset = A::new(source)?;
        let handle = self.gen_handle();
        self.registered_assets.insert(
            handle,
            RegisteredAsset::Unloaded {
                data: asset,
                _t: PhantomData,
            },
        );
        Ok(ResourceBacking::new(handle))
    }

    pub(crate) fn register_asset_gpu_residency(
        &mut self,
        asset_handle: AssetHandle,
        allocation_handle: GPUAllocationHandle,
    ) -> Result<(), AssetLoadError> {
        if let Some(registered_asset) = self.registered_assets.get_mut(&asset_handle) {
            registered_asset.set_as_gpu_loaded(allocation_handle)?;
            return Ok(());
        } else {
            return Err(AssetLoadError::AssetNotFound);
        }
    }

    pub(crate) fn register_asset_gpu_unloaded(
        &mut self,
        asset_handle: AssetHandle,
    ) -> Result<(), AssetLoadError> {
        if let Some(registered_asset) = self.registered_assets.get_mut(&asset_handle) {
            if let RegisteredAsset::Loaded { residency, .. } = registered_asset {
                let AssetResidency::PendingUnloadGPU(_alloc_handle, la_idx) = residency else {
                    panic!("this asset was not pending gpu when you tried to unload it");
                };
                *residency = AssetResidency::CPU(*la_idx);
            }
        } else {
            return Err(AssetLoadError::AssetNotFound);
        }
        return Ok(());
        //if let Some(registered_asset) = self.registered_assets.remove(&asset_handle) {
        //    self.registered_assets
        //        .insert(asset_handle, registered_asset.as_unloaded()?);
        //    return Ok(());
        //} else {
        //    return Err(AssetLoadError::AssetNotFound);
        //}
    }

    pub(crate) fn set_minimum_load_level(
        &mut self,
        asset_handle: &AssetHandle,
        load_level: SceneLoadLevel,
    ) -> Result<AssetResidency, AssetLoadError> {
        let asset_res_level: AssetResidency = self.res_level_of(asset_handle)?;
        match load_level {
            SceneLoadLevel::PendingCPU | SceneLoadLevel::PendingGPU => unreachable!(),
            SceneLoadLevel::NotLoaded => match asset_res_level {
                AssetResidency::CPU(la_idx) => {
                    self.unload(la_idx)?;
                    return Ok(AssetResidency::Registered);
                }
                AssetResidency::GPU(alloc_handle, la_idx) => {
                    return match self.registered_assets.get_mut(asset_handle).unwrap() {
                        RegisteredAsset::Loaded { residency, .. } => {
                            *residency =
                                AssetResidency::PendingUnloadGPU(alloc_handle.clone(), la_idx);
                            Ok(AssetResidency::PendingUnloadGPU(alloc_handle, la_idx))
                        }
                        RegisteredAsset::Unloaded { .. } => {
                            return Err(AssetLoadError::AssetNotLoaded(
                                "this asset was not loaded, but the residency was set as GPU"
                                    .into(),
                            ));
                        }
                    };
                }

                AssetResidency::PendingUnloadGPU(alloc_handle, la_index) => {
                    return Ok(AssetResidency::PendingUnloadGPU(alloc_handle, la_index));
                }
                _ => {
                    todo!()
                }
            },
            SceneLoadLevel::CPU => match asset_res_level {
                AssetResidency::Registered => {
                    let idx = self.load(asset_handle)?;
                    // TODO: start async operation and return PendingCPU
                    return Ok(AssetResidency::CPU(idx));
                }
                AssetResidency::PendingCPU => return Ok(AssetResidency::PendingCPU),
                AssetResidency::PendingGPU(_) => todo!("cancel?"),
                AssetResidency::CPU(idx) => {
                    return Ok(AssetResidency::CPU(idx));
                }
                AssetResidency::GPU(_, _) => todo!("unload gpu?"),
                AssetResidency::PendingUnloadGPU(alloc_handle, la_index) => {
                    return Ok(AssetResidency::PendingUnloadGPU(alloc_handle, la_index));
                }
            },
            SceneLoadLevel::GPU => match asset_res_level {
                AssetResidency::Registered => {
                    let idx = self.load(asset_handle)?;
                    match self.registered_assets.get_mut(asset_handle).unwrap() {
                        RegisteredAsset::Loaded { residency: res, .. } => {
                            *res = AssetResidency::PendingGPU(idx)
                        }
                        _ => panic!("asset not found"),
                    }
                    // TODO: return PendingCPU once async
                    return Ok(AssetResidency::PendingGPU(idx));
                }
                AssetResidency::PendingCPU => return Ok(AssetResidency::PendingCPU),
                AssetResidency::CPU(idx) => {
                    match self.registered_assets.get_mut(asset_handle).unwrap() {
                        RegisteredAsset::Loaded { residency: res, .. } => {
                            *res = AssetResidency::PendingGPU(idx)
                        }
                        _ => panic!("asset not found"),
                    }
                    return Ok(AssetResidency::PendingGPU(idx));
                }
                AssetResidency::PendingGPU(idx) => {
                    return Ok(AssetResidency::PendingGPU(idx));
                }
                AssetResidency::GPU(allocation_handle, idx) => {
                    return Ok(AssetResidency::GPU(allocation_handle.clone(), idx));
                }
                AssetResidency::PendingUnloadGPU(_, _la_index) => {
                    todo!("cancel unload?")
                }
            },
        }
    }

    pub fn get_loaded_asset<'frame>(
        &'frame self,
        asset_handle: &AssetHandle,
    ) -> LoadedAsset<'frame> {
        let a = self
            .registered_assets
            .get(asset_handle)
            .expect("asset is not registered!");

        let RegisteredAsset::Loaded { residency: res, .. } = a else {
            panic!("asset is not loaded!")
        };
        let AssetResidency::GPU(alloc_handle, la_index) = res else {
            panic!("asset is not gpu resident!")
        };
        let asset = &self
            .loaded_assets
            .get(*la_index)
            .expect("loaded asset not found at specified index!")
            .1;
        LoadedAsset::<'frame> {
            asset,
            alloc_handle: alloc_handle.clone(),
        }
    }
}

#[cfg(test)]
pub(super) mod asset_mocks {
    use std::marker::PhantomData;

    use crate::{
        app::GPUAssetUploadJob,
        asset_manager::{
            Asset, AssetHandle, AssetResidency, UnloadedAssetData,
            asset_manager::{AssetManager, RegisteredAsset},
        },
    };

    pub struct MockAsset;
    impl Asset for MockAsset {
        fn new(
            _dir_name: &str,
        ) -> Result<crate::asset_manager::UnloadedAssetData, crate::asset_manager::AssetLoadError>
        where
            Self: Sized,
        {
            Ok(UnloadedAssetData::Mock)
        }

        fn get_upload_job(
            &self,
            asset_handle: crate::asset_manager::AssetHandle,
        ) -> Result<crate::app::GPUAssetUploadJob, crate::asset_manager::AssetLoadError> {
            GPUAssetUploadJob::new(asset_handle, None, None, None, None, None)
        }

        fn as_mesh_provider(&self) -> Option<&dyn crate::asset_manager::ProvidesMeshData> {
            None
        }

        fn as_animation_provider(
            &self,
        ) -> Option<&dyn crate::asset_manager::ProvidesAnimationData> {
            None
        }
    }

    #[cfg(test)]
    impl AssetManager {
        /// Register a handle that has no backing data, pinned at `residency`. Only
        /// `res_level_of` is meaningful for these — there is no loaded asset behind
        /// the index they carry, so they must not be resolved or uploaded.
        pub(crate) fn mock_asset(&mut self, residency: AssetResidency) -> AssetHandle {
            let handle = self.gen_handle();
            self.registered_assets.insert(
                handle,
                RegisteredAsset::Loaded {
                    residency,
                    data: UnloadedAssetData::Mock,
                    _t: PhantomData,
                },
            );
            handle
        }

        /// Move a mock asset to a new residency, standing in for the load queue.
        pub(crate) fn set_mock_residency(
            &mut self,
            asset_handle: &AssetHandle,
            residency: AssetResidency,
        ) {
            let registered = self
                .registered_assets
                .get_mut(asset_handle)
                .expect("mock asset is registered");
            *registered = RegisteredAsset::Loaded {
                residency,
                data: UnloadedAssetData::Mock,
                _t: PhantomData,
            };
        }
    }
}
