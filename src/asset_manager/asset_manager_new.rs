use std::{collections::HashMap, marker::PhantomData};

use crate::{
    app::{GPUAssetUploadJob, renderer::GPUAllocationHandle},
    asset_manager::{
        Asset, AssetHandle, AssetLoadError, AssetLoadResult, AssetResidency, UnloadedAssetData,
    },
    world::{components::ResourceBacking, scene::SceneLoadLevel},
};

enum RegisteredAsset<A: Asset + ?Sized> {
    Unloaded {
        data: UnloadedAssetData,
        _t: PhantomData<A>,
    },
    Loaded(AssetResidency),
}

impl<A: Asset + ?Sized> RegisteredAsset<A> {
    fn set_as_gpu_loaded(&mut self, alloc_handle: GPUAllocationHandle) {
        let Self::Loaded(res) = self else {
            panic!("set gpu called on unloaded asset");
        };
        let AssetResidency::CPU(la_index) = res else {
            panic!("asset residency must be cpu to set loaded");
        };
        *res = AssetResidency::GPU(alloc_handle, *la_index);
    }
}

pub struct AssetManager {
    registered_assets: HashMap<AssetHandle, RegisteredAsset<dyn Asset>>,
    loaded_assets: Vec<Box<dyn Asset>>,
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

    fn res_level_of(&self, asset_handle: &AssetHandle) -> Result<&AssetResidency, AssetLoadError> {
        let registered = self
            .registered_assets
            .get(asset_handle)
            .ok_or(AssetLoadError::AssetNotFound)?;
        match registered {
            RegisteredAsset::Unloaded { data: _data, _t } => Ok(&AssetResidency::Registered),
            RegisteredAsset::Loaded(res) => Ok(res),
        }
    }

    fn load(&mut self, asset_handle: &AssetHandle) -> Result<usize, AssetLoadError> {
        let registered_asset = self.registered_assets.remove(asset_handle).unwrap();
        match registered_asset {
            RegisteredAsset::Unloaded { data, _t } => {
                let loaded = data.load()?;
                let la_index = self.loaded_assets.len().clone();
                self.loaded_assets.push(loaded);
                self.registered_assets.insert(
                    *asset_handle,
                    RegisteredAsset::Loaded(AssetResidency::CPU(la_index)),
                );
                return Ok(la_index);
            }
            RegisteredAsset::Loaded(res) => match res {
                AssetResidency::CPU(la_index) => return Ok(la_index),
                AssetResidency::GPU(_alloc, la_index) => return Ok(la_index),
                _ => panic!(),
            },
        }
    }

    pub fn get_upload_job_for<'a>(
        &'a self,
        asset_handle: AssetHandle,
    ) -> Result<GPUAssetUploadJob<'a>, AssetLoadError> {
        match self.registered_assets.get(&asset_handle).unwrap() {
            RegisteredAsset::Unloaded { data: _data, _t } => Err(AssetLoadError::AssetNotLoaded(
                String::from("this asset is not yet loaded!"),
            )),
            RegisteredAsset::Loaded(res) => match res {
                AssetResidency::CPU(la_index) => {
                    let asset = &self.loaded_assets[*la_index];
                    return asset.get_upload_job(asset_handle);
                }
                _ => return Err(AssetLoadError::AssetNotFound),
            },
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

    pub fn register_asset_gpu_residency(
        &mut self,
        asset_handle: &AssetHandle,
        allocation_handle: GPUAllocationHandle,
    ) -> Result<(), AssetLoadError> {
        if let Some(registered_asset) = self.registered_assets.get_mut(asset_handle) {
            registered_asset.set_as_gpu_loaded(allocation_handle);
            return Ok(());
        } else {
            return Err(AssetLoadError::AssetNotFound);
        }
    }

    pub fn set_minumum_load_level(
        &mut self,
        asset_handle: &AssetHandle,
        load_level: SceneLoadLevel,
    ) -> Result<AssetLoadResult, AssetLoadError> {
        let asset_res_level: &AssetResidency = self.res_level_of(asset_handle)?;
        match load_level {
            SceneLoadLevel::NotLoaded => {
                todo!("unload assets")
            }
            SceneLoadLevel::CPU => match asset_res_level {
                AssetResidency::Registered => {
                    self.load(asset_handle)?;
                    // TODO: start async operation and return PendingCPU
                    return Ok(AssetLoadResult::LoadedCPU);
                }
                AssetResidency::CPU(_) => {
                    return Ok(AssetLoadResult::LoadedCPU);
                }
                AssetResidency::GPU(_, _) => todo!("unload gpu?"),
            },
            SceneLoadLevel::GPU => match asset_res_level {
                AssetResidency::Registered => {
                    self.load(asset_handle)?;
                    // TODO: return PendingCPU once async
                    return Ok(AssetLoadResult::PendingGPU);
                }
                AssetResidency::CPU(_) => {
                    return Ok(AssetLoadResult::PendingGPU);
                }
                AssetResidency::GPU(allocation_handle, _) => {
                    return Ok(AssetLoadResult::LoadedGPU(allocation_handle.clone()));
                }
            },
        }
    }

    pub fn get_loaded_asset(
        &self,
        asset_handle: &AssetHandle,
    ) -> (&GPUAllocationHandle, &Box<dyn Asset>) {
        let a = self
            .registered_assets
            .get(asset_handle)
            .expect("asset is not registered!");
        let RegisteredAsset::Loaded(res) = a else {
            panic!("asset is not loaded!")
        };
        let AssetResidency::GPU(alloc_handle, la_index) = res else {
            panic!("asset is not gpu resident!")
        };
        (
            alloc_handle,
            self.loaded_assets
                .get(*la_index)
                .expect("loaded asset not found at specified index!"),
        )
    }
}
