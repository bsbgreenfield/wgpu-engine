use crate::asset_manager::{gltf_loader::loader::GltfLoadError, model_builder::GltfModelBuilder};
use std::collections::HashMap;

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
pub trait Asset {
    type Builder: AssetBuilder;
    fn builder() -> Self::Builder;
}

pub struct GltfAsset;

impl Asset for GltfAsset {
    type Builder = GltfModelBuilder;
    fn builder() -> Self::Builder {
        GltfModelBuilder::new()
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
    pub fn register<A: Asset>(&mut self) -> Result<(), GltfLoadError>
    where
        A::Builder: 'static,
    {
        let builder = A::builder();
        let handle = self.gen_handle();
        self.asset_registry.insert(handle, Box::new(builder));
        Ok(())
    }
}
