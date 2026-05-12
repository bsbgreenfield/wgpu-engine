use std::fmt::{Debug, Display};

mod test_refactor;

use crate::{
    animation::animation::EntityAnimation,
    app::{GPUAssetUploadJob, renderer::GPUAllocationHandle},
    asset_manager_new::gltf_asset::{
        BinarySource, GltfLoadError, GltfValidationError, LoadedGltfAsset,
    },
    world::{
        RenderKey,
        components::{AnimationAccessor, MeshAcessor, RigidAnimationMode},
        entity_manager::{MeshRenderables, Renderables},
        entity_upload_query::InstanceUploadQueryNew,
        scene::SceneLoadLevel,
        world::RenderView,
    },
};

pub mod asset_manager_new;
pub mod gltf_asset;
mod range_splicer;
#[derive(Debug)]
pub enum AssetLoadError {
    Gltf(GltfLoadError),
    AssetNotLoaded(String),
    AssetNotFound,
    ComponentNotFound,
    NoVertexData,
    InstanceUploadFailure(String),
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
            Self::InstanceUploadFailure(str) => f.write_str(str.as_str()),
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
impl RenderKey for AssetHandle {
    fn as_key(&self) -> u64 {
        self.0 as u64
    }

    fn from_key(key: u64) -> Self {
        Self(key as u32)
    }
}

enum UnloadedAssetData {
    Gltf(gltf::Gltf, BinarySource),
}

impl UnloadedAssetData {
    fn load(self) -> Result<Box<dyn Asset>, ModelBuilderError> {
        match self {
            Self::Gltf(gltf, bin) => LoadedGltfAsset::load(gltf, bin),
        }
    }
}

pub trait Asset {
    fn new(dir_name: &str) -> Result<UnloadedAssetData, AssetLoadError>
    where
        Self: Sized;

    fn get_upload_job(
        &self,
        asset_handle: AssetHandle,
    ) -> Result<GPUAssetUploadJob, AssetLoadError>;

    fn as_mesh_provider(&self) -> Option<&dyn ProvidesMeshData>;
    fn as_animation_provider(&self) -> Option<&dyn ProvidesAnimationData>;
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
pub trait LoadableAsset: Asset {
    fn load(&self) -> Result<Box<dyn LoadedAsset>, ModelBuilderError>;
}
pub trait LoadedAsset {
    fn upload_job<'a>(
        &'a self,
        asset_handle: AssetHandle,
    ) -> Result<GPUAssetUploadJob<'a>, AssetLoadError>;

    fn get_renderables(
        &self,
        alloc_handle: GPUAllocationHandle,
        renderables: &mut Renderables,
        query: &InstanceUploadQueryNew,
    ) -> Result<(), AssetLoadError>;
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

pub trait ProvidesMeshData: Asset {
    fn render_mesh_data<'a>(
        &self,
        mesh_accessor: &'a MeshAcessor,
        mode: &'a RigidAnimationMode,
    ) -> Vec<MeshRenderables>;
}

pub trait ProvidesAnimationData: Asset {
    fn entity_animation<'a>(
        &self,
        animation_accessor: &AnimationAccessor,
        mesh_accessor: &MeshAcessor,
    ) -> Vec<EntityAnimation>;
}
