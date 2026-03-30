use std::fmt::Display;

use crate::asset_manager::gltf_assets::{ModelBuilderError, gltf_loader::GltfLoadError};

pub mod asset_manager;
pub(super) mod gltf_assets;
mod range_splicer;
pub enum Asset {
    Gltf(gltf::Gltf, gltf_assets::gltf_loader::loader::BinarySource),
    Other,
}
#[derive(Debug)]
pub enum AssetLoadError {
    Gltf(GltfLoadError),
    AssetNotLoaded,
    AssetNotFound,
    ComponentNotFound,
}

impl Display for AssetLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gltf(gltf_error) => {
                write!(f, "Asset Load Failure: {}", gltf_error)
            }
            Self::AssetNotLoaded => f.write_str("This asset is not yet loaded"),
            Self::AssetNotFound => f.write_str("No such asset exists"),
            Self::ComponentNotFound => {
                f.write_str("The component associated with this asset does not exist")
            }
        }
    }
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
