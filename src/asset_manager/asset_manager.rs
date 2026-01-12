use crate::asset_manager::{
    gltf_loader::loader::{GltfLoadError, GltfLoader},
    model_builder::GltfModelBuilder,
};
use std::{collections::HashMap, path::PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetHandle(u32);

pub enum AssetLoadError {
    Gltf(GltfLoadError),
}

impl From<GltfLoadError> for AssetLoadError {
    fn from(value: GltfLoadError) -> Self {
        Self::Gltf(value)
    }
}

pub enum AssetType {
    Gltf(String),
}

impl AssetType {
    fn builder(&self) -> Box<dyn AssetBuilder> {
        match self {
            Self::Gltf(string) => Box::new(GltfModelBuilder::new()),
        }
    }
}

pub(super) trait AssetBuilder {
    fn with_asset(&mut self, dir_name: &str) -> Result<(), AssetLoadError>;
}

pub struct AssetManager {
    asset_registry: HashMap<AssetHandle, Box<dyn AssetBuilder>>,
}

impl AssetManager {
    fn gen_handle(&self) -> AssetHandle {
        return AssetHandle(self.asset_registry.len() as u32);
    }
    pub fn register(&mut self, a: AssetType) -> Result<(), GltfLoadError> {
        let asset_builder = a.builder();
        self.asset_registry.insert(self.gen_handle(), asset_builder);
        Ok(())
    }
}
