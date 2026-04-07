use std::fmt::Display;

use crate::asset_manager::{
    asset_manager::AssetResidency,
    gltf_assets::{GltfLoadResult, ModelBuilderError, gltf_loader::GltfLoadError},
};

pub mod asset_manager;
pub(super) mod gltf_assets;
mod range_splicer;
#[derive(Debug)]
pub enum AssetLoadError {
    Gltf(GltfLoadError),
    AssetNotLoaded(String),
    AssetNotFound,
    ComponentNotFound,
}

impl Display for AssetLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gltf(gltf_error) => {
                write!(f, "Asset Load Failure: {}", gltf_error)
            }
            Self::AssetNotLoaded(s) => write!(f, "The asset is not yet loaded. Message: {}", s),
            Self::AssetNotFound => f.write_str("No such asset exists"),
            Self::ComponentNotFound => {
                f.write_str("The component associated with this asset does not exist")
            }
        }
    }
}

impl std::error::Error for AssetLoadError {}

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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AssetHandle {
    id: u32,
}
pub trait Asset {
    fn new(dir_name: &str) -> Result<Self, AssetLoadError>
    where
        Self: Sized;
    fn get_residency_level(&self) -> &AssetResidency;
    fn set_residency_level(&mut self, level: AssetResidency);
    fn load_asset(&self, handle: AssetHandle) -> Result<LoadedAsset, AssetLoadError>;
}

#[derive(Debug)]
pub struct LoadedAsset {
    pub handle: AssetHandle,
    pub gltf_mesh_data: GltfLoadResult,
}

pub trait ModelVertexData {
    fn has_pnu(&self) -> bool;
    fn has_pnujw(&self) -> bool;
}
