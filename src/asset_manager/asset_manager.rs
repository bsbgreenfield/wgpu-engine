use crate::{
    app::render::GPUMeshHandle,
    asset_manager::gltf_assets::{
        gltf_loader::loader::{BinarySource, GltfLoadError, GltfLoader},
        model_builder_new::{GltfBuilder, GltfLoadResult, ModelBuilderError},
    },
    util::types::{IndexType, ModelVertex, PNUJWVertex, PNUVertex},
    world::scene::SceneLoadLevel,
};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AssetHandle {
    id: u32,
}

#[derive(Debug)]
pub enum AssetLoadError {
    Gltf(GltfLoadError),
    AssetNotLoaded,
    AssetNotFound,
    ComponentNotFound,
}

impl From<ModelBuilderError> for AssetLoadError {
    fn from(value: ModelBuilderError) -> Self {
        Self::Gltf(GltfLoadError::ModelBuilderError(Box::new(value)))
    }
}

impl From<GltfLoadError> for AssetLoadError {
    fn from(value: GltfLoadError) -> Self {
        Self::Gltf(value)
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum AssetResidencyLevel {
    Registered,
    CPU(usize),
    GPU,
}
impl PartialEq<SceneLoadLevel> for AssetResidencyLevel {
    fn eq(&self, other: &SceneLoadLevel) -> bool {
        match self {
            AssetResidencyLevel::Registered => {
                if *other == SceneLoadLevel::NotLoaded {
                    return true;
                }
            }
            AssetResidencyLevel::CPU(_) => {
                if *other == SceneLoadLevel::CPU {
                    return true;
                }
            }
            AssetResidencyLevel::GPU => {
                if *other == SceneLoadLevel::GPU {
                    return true;
                }
            }
        }
        return false;
    }
}

impl PartialOrd<SceneLoadLevel> for AssetResidencyLevel {
    fn partial_cmp(&self, other: &SceneLoadLevel) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        match self {
            AssetResidencyLevel::Registered => match other {
                SceneLoadLevel::NotLoaded => return Some(Ordering::Equal),
                SceneLoadLevel::CPU | SceneLoadLevel::GPU => return Some(Ordering::Less),
            },
            AssetResidencyLevel::CPU(_) => match other {
                SceneLoadLevel::NotLoaded => return Some(Ordering::Greater),
                SceneLoadLevel::CPU => return Some(Ordering::Equal),
                SceneLoadLevel::GPU => return Some(Ordering::Less),
            },
            AssetResidencyLevel::GPU => match other {
                SceneLoadLevel::NotLoaded | SceneLoadLevel::CPU => return Some(Ordering::Greater),
                SceneLoadLevel::GPU => return Some(Ordering::Equal),
            },
        }
    }
}

#[derive(Debug)]
pub struct LoadedAsset {
    pub handle: AssetHandle,
    pub gltf_mesh_data: GltfLoadResult,
}

struct CPUVertexData<V: ModelVertex> {
    vertices: Vec<V>,
}
struct CPUIndexData<I: IndexType> {
    indices: Vec<I>,
}

impl<V: ModelVertex> Deref for CPUVertexData<V> {
    type Target = Vec<V>;
    fn deref(&self) -> &Self::Target {
        &self.vertices
    }
}
impl<V: ModelVertex> DerefMut for CPUVertexData<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vertices
    }
}

impl<I: IndexType> Deref for CPUIndexData<I> {
    type Target = Vec<I>;
    fn deref(&self) -> &Self::Target {
        &self.indices
    }
}
impl<I: IndexType> DerefMut for CPUIndexData<I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.indices
    }
}

impl<V: ModelVertex> CPUVertexData<V> {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
        }
    }
}
impl<I: IndexType> CPUIndexData<I> {
    fn new() -> Self {
        Self {
            indices: Vec::new(),
        }
    }
}

pub struct AssetManager {
    registered_handles: Vec<AssetHandle>,
    pnujw_vertex_data: CPUVertexData<PNUJWVertex>,
    pnu_vertex_data: CPUVertexData<PNUVertex>,
    u16_index_data: CPUIndexData<u16>,
    registered_assets: HashMap<AssetHandle, Box<dyn AssetNew>>,
    loaded_assets: Vec<LoadedAsset>,
    load_levels: HashMap<AssetHandle, AssetResidencyLevel>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            registered_handles: Vec::new(),
            pnujw_vertex_data: CPUVertexData::<PNUJWVertex>::new(),
            pnu_vertex_data: CPUVertexData::<PNUVertex>::new(),
            u16_index_data: CPUIndexData::<u16>::new(),
            loaded_assets: Vec::new(),
            registered_assets: HashMap::new(),
            load_levels: HashMap::new(),
        }
    }
    fn gen_handle(&self) -> AssetHandle {
        AssetHandle {
            id: self.registered_handles.len() as u32,
        }
    }
    fn res_level_of(
        &self,
        asset_handle: &AssetHandle,
    ) -> Result<AssetResidencyLevel, AssetLoadError> {
        Ok(self
            .registered_assets
            .get(asset_handle)
            .ok_or(AssetLoadError::AssetNotFound)?
            .get_residency_level())
    }

    pub fn set_minumum_load_level(
        &mut self,
        assets: Vec<AssetHandle>,
        load_level: SceneLoadLevel,
    ) -> Result<SceneLoadLevel, AssetLoadError> {
        let mut loaded_asset_indices = Vec::<usize>::new();
        let mut res = load_level;
        for asset in assets {
            if load_level > SceneLoadLevel::NotLoaded {
                // load asset to CPU if it is not already loaded
                if self.res_level_of(&asset)? < SceneLoadLevel::CPU {
                    let mut registered_asset = self.registered_assets.remove(&asset).unwrap();
                    let loaded_asset: LoadedAsset = registered_asset.load_asset(asset)?;
                    let la_index = self.loaded_assets.len().clone();
                    self.loaded_assets.push(loaded_asset);
                    registered_asset.set_residency_level(AssetResidencyLevel::CPU(la_index));
                    loaded_asset_indices.push(la_index);
                    self.registered_assets.insert(asset, registered_asset);
                    res = SceneLoadLevel::CPU;
                }
            }
        }
        Ok(res)
    }

    pub fn register_asset_gpu_residency(
        &mut self,
        gpu_mesh_handle: &GPUMeshHandle,
    ) -> Result<(), AssetLoadError> {
        self.registered_assets
            .get_mut(&gpu_mesh_handle.handle)
            .ok_or(AssetLoadError::AssetNotFound)?
            .set_residency_level(AssetResidencyLevel::GPU);

        Ok(())
    }

    pub fn get_loaded_assets(&self, handles: Vec<AssetHandle>) -> Vec<&LoadedAsset> {
        let mut loaded_asset_refs = Vec::new();
        for handle in handles {
            let a = self.registered_assets.get(&handle).unwrap();
            let res_level = a.get_residency_level();
            match res_level {
                AssetResidencyLevel::CPU(la_index) => {
                    loaded_asset_refs.push(&self.loaded_assets[la_index]);
                }
                _ => {}
            }
        }
        loaded_asset_refs
    }

    pub fn register_asset<A>(&mut self, source: &str) -> Result<AssetHandle, AssetLoadError>
    where
        A: AssetNew + 'static,
    {
        let asset = A::new(source)?;
        let handle = self.gen_handle();
        self.registered_assets.insert(handle, Box::new(asset));
        todo!("rest of the function goes here")
    }
}

pub trait AssetNew {
    fn new(dir_name: &str) -> Result<Self, AssetLoadError>
    where
        Self: Sized;
    fn get_residency_level(&self) -> AssetResidencyLevel;
    fn set_residency_level(&mut self, level: AssetResidencyLevel);
    fn load_asset(&self, handle: AssetHandle) -> Result<LoadedAsset, AssetLoadError>;
}

pub struct GltfAsset {
    gltf: gltf::Gltf,
    bin: BinarySource,
    res_level: AssetResidencyLevel,
}
impl GltfBuilder for GltfAsset {}

impl AssetNew for GltfAsset {
    fn get_residency_level(&self) -> AssetResidencyLevel {
        self.res_level
    }
    fn set_residency_level(&mut self, level: AssetResidencyLevel) {
        self.res_level = level;
    }
    fn new(dir_name: &str) -> Result<Self, AssetLoadError>
    where
        Self: Sized,
    {
        let (gltf, bin) = GltfLoader::load_gltf_from_resource(dir_name)?;
        Ok(Self {
            gltf,
            bin,
            res_level: AssetResidencyLevel::Registered,
        })
    }
    fn load_asset(&self, handle: AssetHandle) -> Result<LoadedAsset, AssetLoadError> {
        let a = Self::load_gltf(&self.gltf, &self.bin).unwrap();
        Ok(LoadedAsset {
            gltf_mesh_data: a,
            handle,
        })
    }
}
