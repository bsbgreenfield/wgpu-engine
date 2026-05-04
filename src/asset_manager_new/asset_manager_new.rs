use std::collections::HashMap;

use crate::{
    app::{GPUAssetUploadJob, renderer::GPUAllocationHandle},
    asset_manager_new::{
        Asset, AssetHandle, AssetLoadError, AssetLoadResult, AssetResidency, LoadableAsset,
        LoadedAsset,
    },
    world::{
        InstanceUploadQuery, components::MeshCollectionComponent, entity_manager::Renderables,
        scene::SceneLoadLevel,
    },
};

struct RegisteredAsset {
    residency_level: AssetResidency,
    asset: Box<dyn LoadableAsset>,
}

impl RegisteredAsset {
    fn new<A: Asset + LoadableAsset + 'static>(asset: A) -> Self {
        Self {
            residency_level: AssetResidency::Registered,
            asset: Box::new(asset),
        }
    }
}

pub struct AssetManagerNew {
    registered_assets: HashMap<AssetHandle, RegisteredAsset>,
    loaded_assets: Vec<Box<dyn LoadedAsset>>,
}

impl AssetManagerNew {
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
        Ok(&self
            .registered_assets
            .get(asset_handle)
            .ok_or(AssetLoadError::AssetNotFound)?
            .residency_level)
    }

    fn load(&mut self, asset_handle: &AssetHandle) -> Result<usize, AssetLoadError> {
        let mut registered_asset = self.registered_assets.remove(asset_handle).unwrap();

        let loaded_asset: Box<dyn LoadedAsset> = registered_asset.asset.load()?;
        let la_index = self.loaded_assets.len().clone();
        self.loaded_assets.push(loaded_asset);
        registered_asset.residency_level = AssetResidency::CPU(la_index);
        self.registered_assets
            .insert(*asset_handle, registered_asset);
        Ok(la_index)
    }

    pub fn get_upload_job_for<'a>(
        &'a self,
        asset_handle: AssetHandle,
    ) -> Result<GPUAssetUploadJob<'a>, AssetLoadError> {
        match self
            .registered_assets
            .get(&asset_handle)
            .unwrap()
            .residency_level
        {
            AssetResidency::CPU(la_index) => {
                let la = &self.loaded_assets[la_index];
                return la.upload_job(asset_handle);
            }
            _ => return Err(AssetLoadError::AssetNotFound),
        }
    }
    pub fn register_asset<A>(&mut self, source: &str) -> Result<AssetHandle, AssetLoadError>
    where
        A: Asset + LoadableAsset + 'static,
    {
        let asset = A::new(source)?;
        let handle = self.gen_handle();
        self.registered_assets
            .insert(handle, RegisteredAsset::new(asset));
        Ok(handle)
    }

    pub fn register_asset_gpu_residency(
        &mut self,
        asset_handle: &AssetHandle,
        allocation_handle: GPUAllocationHandle,
    ) -> Result<(), AssetLoadError> {
        if let Some(registered_asset) = self.registered_assets.get_mut(asset_handle) {
            match registered_asset.residency_level {
                AssetResidency::CPU(la_index) => {
                    registered_asset.residency_level =
                        AssetResidency::GPU(allocation_handle, la_index);
                    return Ok(());
                }
                _ => {
                    return Err(AssetLoadError::AssetNotLoaded(String::from(
                        "tried to register asset GPU resident but it was not CPU resident",
                    )));
                }
            }
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

    pub fn get_renderables_for(
        &self,
        asset_handle: &AssetHandle,
        renderables: &mut Renderables,
        query: &InstanceUploadQuery,
    ) -> Result<(), AssetLoadError> {
        match &self
            .registered_assets
            .get(asset_handle)
            .unwrap()
            .residency_level
        {
            AssetResidency::GPU(aloc_handle, la_index) => {
                let la = &self.loaded_assets[*la_index];
                la.get_renderables(aloc_handle.clone(), renderables, query)
            }
            AssetResidency::Registered => {
                panic!("this mesh_collection_component is not yet loaded")
            }
            AssetResidency::CPU(_) => {
                panic!(
                    "this mesh_collection_component is not GPU resident, so its not ready to spawn!"
                )
            }
        }
    }

    fn unwrap_la(&self, asset_handle: &AssetHandle) -> &Box<dyn LoadedAsset> {
        match self
            .registered_assets
            .get(asset_handle)
            .unwrap()
            .residency_level
        {
            AssetResidency::GPU(_, la_index) => &self.loaded_assets[la_index],
            _ => panic!("asset is not gpu resident"),
        }
    }

    // pub fn get_instanced_upload_data_for(
    //     &self,
    //     asset_handle: &AssetHandle,
    //     instance_handle: InstanceHandle,
    //     mesh_accessor: &MeshAcessor,
    // ) -> InstanceUploadData {
    //     let la = self.unwrap_la(asset_handle);
    //     la.get_instance_upload_data(instance_handle, mesh_accessor)
    // }
}
