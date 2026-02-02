use crate::{
    asset_manager::gltf_assets::{
        gltf_loader::loader::GltfLoadError,
        model_builder_new::{MeshCollectionAssetData, ModelBuilderError},
    },
    util::types::{IndexType, ModelVertex, PNUJWVertex},
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    ops::Range,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AssetHandle {
    type_id: TypeId,
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
pub trait Asset {
    type Builder: AssetBuilder;
    type Components;
    fn builder(dir_name: &str) -> Result<Self::Builder, AssetLoadError>;
}

#[derive(Clone, Copy)]
pub enum AssetResidencyLevel {
    Registered,
    CPU,
    GPU,
}

struct AssetLoadResult {
    vertex_type: TypeId,
    index_type: TypeId,
}

pub trait AssetBuilder {
    fn load_asset(
        &mut self,
        vertex_data_offset: usize,
        index_data_offset: usize,
    ) -> Result<(Vec<u8>, Vec<u8>), AssetLoadError>;
    fn get_residency_level(&self) -> AssetResidencyLevel;

    fn get_vertex_format(&self) -> TypeId;
    fn get_index_format(&self) -> TypeId;
}

#[derive(Debug)]
pub struct LoadedAsset {
    mesh_collections: Vec<MeshCollectionAssetData>,
}

impl LoadedAsset {
    pub fn new() -> Self {
        Self {
            mesh_collections: Vec::new(),
        }
    }

    pub fn add_mesh_collections(&mut self, mesh_collections: Vec<MeshCollectionAssetData>) {
        self.mesh_collections.extend(mesh_collections);
    }
}

struct CPUVertexData<V: ModelVertex> {
    vertices: Vec<u8>,
    vertex_type: PhantomData<V>,
}
struct CPUIndexData<I: IndexType> {
    indices: Vec<u8>,
    index_type: PhantomData<I>,
}

impl<V: ModelVertex> CPUVertexData<V> {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            vertex_type: PhantomData::<V>,
        }
    }
}
impl<I: IndexType> CPUIndexData<I> {
    fn new() -> Self {
        Self {
            indices: Vec::new(),
            index_type: PhantomData::<I>,
        }
    }
}

pub struct AssetManager {
    PNUJW_cpu_data: CPUVertexData<PNUJWVertex>,
    U16_index_data: CPUIndexData<u16>,
    asset_registry: HashMap<u32, Box<dyn AssetBuilder>>,
    asset_data: HashMap<u32, LoadedAsset>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            PNUJW_cpu_data: CPUVertexData::<PNUJWVertex>::new(),
            U16_index_data: CPUIndexData::<u16>::new(),
            asset_registry: HashMap::new(),
            asset_data: HashMap::new(),
        }
    }
    pub fn get_builder(&self, asset_handle: &AssetHandle) -> Option<&Box<dyn AssetBuilder>> {
        self.asset_registry.get(&asset_handle.id)
    }
    fn gen_handle<A: Asset + 'static>(&self) -> AssetHandle {
        AssetHandle {
            type_id: TypeId::of::<A>(),
            id: self.asset_registry.len() as u32,
        }
    }

    pub fn set_minumum_load_level(&mut self, assets: Vec<AssetHandle>) {
        for asset in assets {
            if let Some(a) = self.asset_registry.get_mut(&asset.id) {
                a.load_asset(self, 0, 0);
            }
        }
    }

    // pub fn get_components_for(
    //     &mut self,
    //     asset_handle: &AssetHandle,
    // ) -> Result<&LoadedAsset, AssetLoadError> {
    //     let builder = self
    //         .asset_registry
    //         .get(&asset_handle.id)
    //         .ok_or(AssetLoadError::AssetNotLoaded)?;

    //     let la = builder.get_components()?;
    //     self.asset_data.insert(asset_handle.id, la);
    //     Ok(self.asset_data.get(&asset_handle.id).unwrap())
    // }

    pub fn register_asset<A: Asset + 'static>(
        &mut self,
        dir_name: &str,
    ) -> Result<AssetHandle, AssetLoadError>
    where
        A::Builder: AssetBuilder + 'static,
    {
        let builder = A::builder(dir_name)?;
        let handle = self.gen_handle::<A>();
        self.asset_registry.insert(handle.id, Box::new(builder));
        Ok(handle)
    }
}
