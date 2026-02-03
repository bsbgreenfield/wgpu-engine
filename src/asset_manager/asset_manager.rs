use crate::{
    asset_manager::{
        asset_builder_new::AssetBuilderNew,
        gltf_assets::{
            gltf_loader::loader::GltfLoadError,
            model_builder_new::{MeshCollectionAssetData, ModelBuilderError},
        },
    },
    util::types::{IndexType, ModelVertex, PNUJWVertex},
};
use std::{any::TypeId, collections::HashMap};

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

#[derive(Clone, Copy)]
pub enum AssetResidencyLevel {
    Registered,
    CPU,
    GPU,
}

pub trait AssetBuilder<V: ModelVertex, I: IndexType> {
    fn load_asset(
        &self,
        vertex_data: &mut CPUVertexData<V>,
        index_data: &mut CPUIndexData<I>,
    ) -> Result<LoadedAsset, AssetLoadError>;
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
    vertices: Vec<V>,
}
struct CPUIndexData<I: IndexType> {
    indices: Vec<I>,
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
    PNUJW_cpu_data: CPUVertexData<PNUJWVertex>,
    U16_index_data: CPUIndexData<u16>,
    PNUJW_U16_Builder: AssetBuilderNew<PNUJWVertex, u16>,
    asset_data: HashMap<u32, LoadedAsset>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            registered_handles: Vec::new(),
            PNUJW_cpu_data: CPUVertexData::<PNUJWVertex>::new(),
            U16_index_data: CPUIndexData::<u16>::new(),
            PNUJW_U16_Builder: AssetBuilderNew::new(),
            asset_data: HashMap::new(),
        }
    }
    fn gen_handle(&self) -> AssetHandle {
        AssetHandle {
            id: self.registered_handles.len() as u32,
        }
    }

    pub fn set_minumum_load_level(&mut self, assets: Vec<AssetHandle>) {
        todo!()
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

    pub fn register_asset<V: ModelVertex, I: IndexType>(
        &mut self,
        asset: super::Asset,
    ) -> Result<AssetHandle, AssetLoadError> {
        let builder = self.get_builder();
        let handle = AssetHandle { id: 0 };
        builder.register_asset(asset, handle);
        Ok(handle)
    }
}

trait BuilderSelector<V: ModelVertex, I: IndexType> {
    fn get_builder(&mut self) -> &mut AssetBuilderNew<V, I>;
}

impl BuilderSelector<PNUJWVertex, u16> for AssetManager {
    fn get_builder(&mut self) -> &mut AssetBuilderNew<PNUJWVertex, u16> {
        &mut self.PNUJW_U16_Builder
    }
}
