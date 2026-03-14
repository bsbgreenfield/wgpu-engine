use crate::{
    app::renderer_new::GPUAllocationHandle,
    asset_manager::gltf_assets::{
        gltf_loader::loader::{BinarySource, GltfLoadError, GltfLoader},
        model_builder_new::{GltfBuilder, GltfLoadResult, GltfMeshData, ModelBuilderError},
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

#[derive(Clone)]
pub enum AssetResidency {
    Registered,
    CPU(usize),
    GPU(GPUAllocationHandle),
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
            AssetResidency::GPU(_) => {
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
            AssetResidency::GPU(_) => match other {
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

use std::any::TypeId;
use std::ops::Range;
impl LoadedAsset {
    pub fn mesh_ids_and_prim_ranges_of<V: ModelVertex>(&self) -> (Vec<u32>, Vec<Range<u32>>) {
        let mut mesh_ids = Vec::<u32>::new();
        let mut primitive_ranges = Vec::<Range<u32>>::new();
        for mesh_data in self.gltf_mesh_data.mesh_data.iter() {
            // find all meshes which contain primitives of the correct type
            let filtered_meshes = mesh_data.meshes.iter().filter(|m| {
                m.primitives
                    .iter()
                    .any(|p| p.vertex_type == TypeId::of::<V>())
            });
            for filtered_mesh in filtered_meshes {
                for candidate_primitive in filtered_mesh.primitives.iter() {
                    if candidate_primitive.vertex_type == TypeId::of::<V>() {
                        mesh_ids.push(filtered_mesh.id);
                        // ADD PRIMITIVE COUNT FOR MODEL IF NECESSARY HERE
                        primitive_ranges.push(candidate_primitive.vertices.clone());
                    }
                }
            }
        }
        (mesh_ids, primitive_ranges)
    }
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

pub enum AssetLoadResult {
    LoadedCPU,
    LoadedGPU(GPUAllocationHandle),
    PendingGPU,
    PendingCPU,
}

pub struct AssetManager {
    registered_handles: Vec<AssetHandle>,
    registered_assets: HashMap<AssetHandle, Box<dyn AssetNew>>,
    loaded_assets: Vec<LoadedAsset>,
    load_levels: HashMap<AssetHandle, AssetResidency>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            registered_handles: Vec::new(),
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
    fn res_level_of(&self, asset_handle: &AssetHandle) -> Result<&AssetResidency, AssetLoadError> {
        Ok(self
            .registered_assets
            .get(asset_handle)
            .ok_or(AssetLoadError::AssetNotFound)?
            .get_residency_level())
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
                AssetResidency::CPU(la_id) => {
                    return Ok(AssetLoadResult::LoadedCPU);
                }
                AssetResidency::GPU(_) => todo!("unload gpu?"),
            },
            SceneLoadLevel::GPU => match asset_res_level {
                AssetResidency::Registered => {
                    // TODO: return PendingCPU once async
                    return Ok(AssetLoadResult::PendingGPU);
                }
                AssetResidency::CPU(loaded_asset_id) => {
                    return Ok(AssetLoadResult::PendingGPU);
                }
                AssetResidency::GPU(allocation_handle) => {
                    return Ok(AssetLoadResult::LoadedGPU(allocation_handle.clone()));
                }
            },
        }
    }

    pub fn register_asset_gpu_residency(
        &mut self,
        gpu_handle: &GPUAllocationHandle,
    ) -> Result<(), AssetLoadError> {
        self.registered_assets
            .get_mut(&gpu_handle.asset_handle)
            .ok_or(AssetLoadError::AssetNotFound)?
            .set_residency_level(AssetResidency::GPU(gpu_handle.clone()));

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
            _ => None,
        }
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
    fn get_residency_level(&self) -> &AssetResidency;
    fn set_residency_level(&mut self, level: AssetResidency);
    fn load_asset(&self, handle: AssetHandle) -> Result<LoadedAsset, AssetLoadError>;
}

pub struct GltfAsset {
    gltf: gltf::Gltf,
    bin: BinarySource,
    res_level: AssetResidency,
}
impl GltfBuilder for GltfAsset {}

impl AssetNew for GltfAsset {
    fn get_residency_level(&self) -> &AssetResidency {
        &self.res_level
    }
    fn set_residency_level(&mut self, level: AssetResidency) {
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
            res_level: AssetResidency::Registered,
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
