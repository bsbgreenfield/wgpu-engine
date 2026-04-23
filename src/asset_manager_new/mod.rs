use std::fmt::{Debug, Display};

use crate::{
    app::{GPUUploadJob, renderer::GPUAllocationHandle},
    asset_manager_new::gltf::{GltfLoadError, GltfValidationError},
    world::{
        entity_manager::{InstanceRenderData, Renderables},
        scene::SceneLoadLevel,
    },
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
    NoVertexData,
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
            Self::NoVertexData => f.write_str("This Asset has no vertices to upload"),
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
    fn upload_job<'a>(
        &'a self,
        asset_handle: &'a AssetHandle,
    ) -> Result<GPUUploadJob<'a>, AssetLoadError>;

    fn get_renderables(&self, alloc_handle: GPUAllocationHandle) -> Vec<InstanceRenderData>;
}
#[derive(Debug)]
pub enum ModelBuilderError {
    NodeNotFound(usize),
    MeshNotFound(usize),
    ValidationError(GltfValidationError),
    BinarySourceNotFound,
    IndexRangeError,
}

impl Display for ModelBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeNotFound(node_id) => write!(f, "Node {} not found", node_id),
            Self::MeshNotFound(mesh_id) => write!(f, "Could not resolve mesh {}", mesh_id),
            Self::ValidationError(err) => err.fmt(f),
            Self::BinarySourceNotFound => f.write_str("binary source not found"),
            Self::IndexRangeError => f.write_str("index range out of bounds"),
        }
    }
}

impl std::error::Error for ModelBuilderError {}

impl From<GltfValidationError> for ModelBuilderError {
    fn from(value: GltfValidationError) -> Self {
        Self::ValidationError(value)
    }
}
