use std::fmt::Display;

use crate::{
    app::{GPUUploadJob, renderer::GPUAllocationHandle},
    asset_manager::{ModelBuilderError, gltf_asset::GltfLoadError},
    world::{entity_manager::Renderables, scene::SceneLoadLevel},
};

pub mod asset_manager_new;
pub mod gltf;
mod range_splicer;
#[derive(Debug)]
pub enum AssetLoadError {
    Gltf(GltfLoadError),
    AssetNotLoaded(String),
    AssetNotFound,
    ComponentNotFound,
}
#[derive(Debug)]
pub enum AssetLoadResult {
    LoadedCPU,
    LoadedGPU(GPUAllocationHandle),
    PendingGPU,
    PendingCPU,
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

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct AssetHandle(u32);

pub trait Asset {
    fn new(dir_name: &str) -> Result<Self, AssetLoadError>
    where
        Self: Sized;
}
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
                SceneLoadLevel::CPU | SceneLoadLevel::GPU => {
                    return Some(Ordering::Less);
                }
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
trait LoadableAsset: Asset {
    fn load(&self) -> Result<Box<dyn LoadedAsset>, ModelBuilderError>;
}
trait LoadedAsset {
    fn upload_job<'a>(&'a self) -> Result<GPUUploadJob<'a>, AssetLoadError>;

    fn get_renderables(&self) -> Option<Renderables>;
}
