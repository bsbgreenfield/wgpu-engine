use std::{collections::HashMap, fmt::Debug, marker::PhantomData};

use crate::{
    app::GPUAssetUploadJob,
    asset_manager::{
        Asset, AssetHandle, AssetLoadError, AssetResidency, LoadedAsset, UnloadedAssetData,
    },
    renderer::GPUAllocationHandle,
    world::{entity_manager::components::ResourceBacking, scene::SceneLoadLevel},
};

enum RegisteredAsset<A: Asset + ?Sized> {
    Unloaded {
        data: UnloadedAssetData,
        _t: PhantomData<A>,
    },
    Loaded(AssetResidency),
}
impl<A: Asset + ?Sized> Debug for RegisteredAsset<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unloaded { data, _t } => f
                .debug_struct("Unloaded")
                .field("data", data)
                .field("_t", _t)
                .finish(),
            Self::Loaded(arg0) => f.debug_tuple("Loaded").field(arg0).finish(),
        }
    }
}

impl<A: Asset + ?Sized> RegisteredAsset<A> {
    fn set_as_gpu_loaded(&mut self, alloc_handle: GPUAllocationHandle) {
        let Self::Loaded(res) = self else {
            panic!("set gpu called on unloaded asset");
        };
        if let AssetResidency::PendingGPU(idx) = res {
            *res = AssetResidency::GPU(alloc_handle, *idx);
        } else {
            panic!("tried to set gpu loaded on asset with residency of {res:?}");
        }
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

    pub fn res_level_of(
        &self,
        asset_handle: &AssetHandle,
    ) -> Result<AssetResidency, AssetLoadError> {
        let registered = self
            .registered_assets
            .get(asset_handle)
            .ok_or(AssetLoadError::AssetNotFound)?;
        match registered {
            RegisteredAsset::Unloaded { data: _data, _t } => Ok(AssetResidency::Registered),
            RegisteredAsset::Loaded(res) => Ok(res.clone()),
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
    ) -> Result<GPUAssetUploadJob, AssetLoadError> {
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
    ) -> Result<AssetResidency, AssetLoadError> {
        let asset_res_level: AssetResidency = self.res_level_of(asset_handle)?;
        match load_level {
            SceneLoadLevel::NotLoaded => {
                todo!("unload assets")
            }
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
            },
            SceneLoadLevel::GPU => match asset_res_level {
                AssetResidency::Registered => {
                    let idx = self.load(asset_handle)?;
                    match self.registered_assets.get_mut(asset_handle).unwrap() {
                        RegisteredAsset::Loaded(res) => *res = AssetResidency::PendingGPU(idx),
                        _ => panic!("asset not found"),
                    }
                    // TODO: return PendingCPU once async
                    return Ok(AssetResidency::PendingGPU(idx));
                }
                AssetResidency::PendingCPU => return Ok(AssetResidency::PendingCPU),
                AssetResidency::CPU(idx) => {
                    match self.registered_assets.get_mut(asset_handle).unwrap() {
                        RegisteredAsset::Loaded(res) => *res = AssetResidency::PendingGPU(idx),
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

        let RegisteredAsset::Loaded(res) = a else {
            panic!("asset is not loaded!")
        };
        let AssetResidency::GPU(alloc_handle, la_index) = res else {
            panic!("asset is not gpu resident!")
        };
        let asset = self
            .loaded_assets
            .get(*la_index)
            .expect("loaded asset not found at specified index!");
        LoadedAsset::<'frame> {
            asset,
            alloc_handle: alloc_handle.clone(),
        }
    }

    pub fn resolve_asset_handle(&self, asset_handle: &AssetHandle) -> GPUAllocationHandle {
        let a = self
            .registered_assets
            .get(asset_handle)
            .expect("should be registered");
        let RegisteredAsset::Loaded(res) = a else {
            panic!("asset is not loaded!")
        };
        let AssetResidency::GPU(alloc_handle, _la_index) = res else {
            panic!("asset is not gpu resident!")
        };
        alloc_handle.clone()
    }
}
