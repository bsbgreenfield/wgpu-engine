use crate::{
    asset_manager::{
        gltf_loader::loader::GltfLoadError,
        model_builder::{GltfBuilderRegistered, GltfModelBuilder},
    },
    world::components::{ExtractComponents, MeshCollectionComponent},
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetHandle {
    type_id: TypeId,
    id: u32,
}

#[derive(Debug)]
pub enum AssetLoadError {
    Gltf(GltfLoadError),
    AssetNotLoaded,
    AssetNotFound,
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

pub struct GltfAsset;

impl Asset for GltfAsset {
    type Builder = GltfBuilderRegistered;
    type Components = (MeshCollectionComponent,);
    fn builder(dir_name: &str) -> Result<Self::Builder, AssetLoadError> {
        GltfBuilderRegistered::new(dir_name)
    }
}

#[derive(Clone, Copy)]
pub enum AssetResidencyLevel {
    Registered,
    CPU,
    GPU,
}

pub trait AssetBuilder {
    fn load_asset(self) -> Result<Box<dyn AssetBuilder>, AssetLoadError>;
    fn get_residency_level(&self) -> AssetResidencyLevel;
    fn get_components(&self) -> Result<Vec<LoadedAsset>, AssetLoadError>;
}

#[derive(Debug)]
pub struct LoadedAsset {
    components: HashMap<TypeId, Box<dyn Any>>,
}

impl LoadedAsset {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    pub fn add_component(&mut self, component: Box<dyn Any>) {
        self.components.insert(component.type_id(), component);
    }

    pub fn get(&mut self, tid: &TypeId) -> Option<Box<dyn Any>> {
        self.components.remove(tid)
    }
}

pub struct AssetManager {
    asset_registry: HashMap<u32, Box<dyn AssetBuilder>>,
    asset_data: HashMap<u32, LoadedAsset>,
}

impl AssetManager {
    pub fn get_builder(&self, asset_handle: &AssetHandle) -> Option<&Box<dyn AssetBuilder>> {
        self.asset_registry.get(&asset_handle.id)
    }
    fn gen_handle<A: Asset + 'static>(&self) -> AssetHandle {
        AssetHandle {
            type_id: TypeId::of::<A>(),
            id: self.asset_registry.len() as u32,
        }
    }

    fn get_components_for(
        &self,
        asset_handle: &AssetHandle,
    ) -> Result<Vec<LoadedAsset>, AssetLoadError> {
        let builder = self
            .asset_registry
            .get(&asset_handle.id)
            .ok_or(AssetLoadError::AssetNotLoaded)?;

        builder.get_components()
    }

    fn register_with_asset<A: Asset + 'static>(
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
