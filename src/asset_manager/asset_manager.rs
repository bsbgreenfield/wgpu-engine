use crate::{
    app::renderer::GPUAllocationHandle,
    asset_manager::{Asset, AssetHandle, AssetLoadError, LoadedAsset},
    world::scene::SceneLoadLevel,
};
use std::collections::HashMap;

#[derive(Clone)]
pub enum AssetResidency {
    Registered,
    CPU(usize),
    GPU(GPUAllocationHandle, usize),
}
impl PartialEq<SceneLoadLevel> for AssetResidency {
    fn eq(&self, other: &SceneLoadLevel) -> bool {
        match self {
            AssetResidency::Registered => {
                if *other == SceneLoadLevel::NotLoaded {
                    return true;
                }
            }
            AssetResidency::CPU(_) => {
                if *other == SceneLoadLevel::CPU {
                    return true;
                }
            }
            AssetResidency::GPU(_, _) => {
                if *other == SceneLoadLevel::GPU {
                    return true;
                }
            }
        }
        return false;
    }
}

impl PartialOrd<SceneLoadLevel> for AssetResidency {
    fn partial_cmp(&self, other: &SceneLoadLevel) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        match self {
            AssetResidency::Registered => match other {
                SceneLoadLevel::NotLoaded => return Some(Ordering::Equal),
                SceneLoadLevel::CPU | SceneLoadLevel::GPU => return Some(Ordering::Less),
            },
            AssetResidency::CPU(_) => match other {
                SceneLoadLevel::NotLoaded => return Some(Ordering::Greater),
                SceneLoadLevel::CPU => return Some(Ordering::Equal),
                SceneLoadLevel::GPU => return Some(Ordering::Less),
            },
            AssetResidency::GPU(_, _) => match other {
                SceneLoadLevel::NotLoaded | SceneLoadLevel::CPU => return Some(Ordering::Greater),
                SceneLoadLevel::GPU => return Some(Ordering::Equal),
            },
        }
    }
}

#[derive(Debug)]
pub enum AssetLoadResult {
    LoadedCPU,
    LoadedGPU(GPUAllocationHandle),
    PendingGPU,
    PendingCPU,
}

impl AssetLoadResult {
    pub fn is_greater_than_or_equal_to(&self, load_level: SceneLoadLevel) -> bool {
        match load_level {
            SceneLoadLevel::NotLoaded => true,
            SceneLoadLevel::CPU => match self {
                Self::PendingCPU => false,
                _ => true,
            },
            SceneLoadLevel::GPU => match self {
                Self::LoadedGPU(_) => true,
                _ => false,
            },
        }
    }
}

pub struct AssetManager {
    registered_assets: HashMap<AssetHandle, Box<dyn Asset>>,
    loaded_assets: Vec<LoadedAsset>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            loaded_assets: Vec::new(),
            registered_assets: HashMap::new(),
        }
    }
    fn gen_handle(&self) -> AssetHandle {
        AssetHandle {
            id: self.registered_assets.len() as u32,
        }
    }
    fn res_level_of(&self, asset_handle: &AssetHandle) -> Result<&AssetResidency, AssetLoadError> {
        Ok(self
            .registered_assets
            .get(asset_handle)
            .ok_or(AssetLoadError::AssetNotFound)?
            .get_residency_level())
    }

    pub fn get_alloc_handle_of(&self, asset_handle: &AssetHandle) -> GPUAllocationHandle {
        match self.res_level_of(asset_handle).expect("couldnt find asset") {
            AssetResidency::GPU(gpu_handle, _) => gpu_handle.clone(),
            _ => panic!(
                "attempted to extract a gpu handle from an asset that hasnt been GPU loaded!"
            ),
        }
    }

    fn load_cpu(&mut self, asset: AssetHandle) -> Result<usize, AssetLoadError> {
        let mut registered_asset = self.registered_assets.remove(&asset).unwrap();
        let loaded_asset: LoadedAsset = registered_asset.load_asset(asset)?;
        let la_index = self.loaded_assets.len().clone();
        self.loaded_assets.push(loaded_asset);
        registered_asset.set_residency_level(AssetResidency::CPU(la_index));
        self.registered_assets.insert(asset, registered_asset);
        Ok(la_index)
    }

    pub fn set_minumum_load_level(
        &mut self,
        asset: AssetHandle,
        load_level: SceneLoadLevel,
    ) -> Result<AssetLoadResult, AssetLoadError> {
        let asset_res_level: &AssetResidency = self.res_level_of(&asset)?;
        match load_level {
            SceneLoadLevel::NotLoaded => {
                todo!("unload assets")
            }
            SceneLoadLevel::CPU => match asset_res_level {
                AssetResidency::Registered => {
                    self.load_cpu(asset)?;
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
                    self.load_cpu(asset)?;
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

    pub fn register_asset_gpu_residency(
        &mut self,
        gpu_handle: &GPUAllocationHandle,
    ) -> Result<(), AssetLoadError> {
        match self.registered_assets.get_mut(&gpu_handle.asset_handle) {
            Some(asset) => match asset.get_residency_level() {
                AssetResidency::CPU(la_index) => {
                    asset.set_residency_level(AssetResidency::GPU(gpu_handle.clone(), *la_index))
                }
                _ => {
                    return Err(AssetLoadError::AssetNotLoaded(String::from(
                        "the asset hasnt been registered as CPU loaded before attmepting to mark it as GPU loaded",
                    )));
                }
            },
            None => return Err(AssetLoadError::AssetNotFound),
        }

        Ok(())
    }

    pub fn get_loaded_assets(&self, handles: Vec<AssetHandle>) -> Vec<&LoadedAsset> {
        let mut loaded_asset_refs = Vec::new();
        for handle in handles {
            let a = self.registered_assets.get(&handle).unwrap();
            let res_level = a.get_residency_level();
            match res_level {
                AssetResidency::CPU(la_index) => {
                    loaded_asset_refs.push(&self.loaded_assets[*la_index]);
                }
                _ => {}
            }
        }
        loaded_asset_refs
    }

    pub fn get_loaded_asset(&self, handle: &AssetHandle) -> Option<&LoadedAsset> {
        let res_level = self
            .registered_assets
            .get(handle)
            .unwrap()
            .get_residency_level();
        match res_level {
            AssetResidency::CPU(la_index) => return Some(&self.loaded_assets[*la_index]),
            AssetResidency::GPU(_, la_index) => {
                return Some(&self.loaded_assets[*la_index]);
            }
            AssetResidency::Registered => return None,
        };
    }

    pub fn register_asset<A>(&mut self, source: &str) -> Result<AssetHandle, AssetLoadError>
    where
        A: Asset + 'static,
    {
        let asset = A::new(source)?;
        let handle = self.gen_handle();
        println!("registered len: {}", self.registered_assets.len());
        self.registered_assets.insert(handle, Box::new(asset));

        Ok(handle)
    }
}

#[cfg(test)]
mod asset_manager_tests {
    use crate::asset_manager::{
        asset_manager::{AssetManager, AssetResidency},
        gltf_assets::GltfAsset,
    };

    #[test]
    fn register_asset() {
        let mut manager = AssetManager::new();
        let asset = manager.register_asset::<GltfAsset>("box").unwrap();
        let a = manager.registered_assets.get(&asset).unwrap();
        assert!(matches!(
            *a.get_residency_level(),
            AssetResidency::Registered
        ))
    }
}
