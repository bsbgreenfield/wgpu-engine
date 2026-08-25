use std::{
    fmt::{Debug, Display},
    ops::Deref,
    range::Range,
};

use crate::{
    animation::EntityAnimationData,
    app::GPUAssetUploadJob,
    asset_manager::gltf_asset::{BinarySource, GltfAsset, GltfLoadError, GltfValidationError},
    renderer::GPUAllocationHandle,
    util::types::{LocalTransform, Mat4F32},
    world::{
        RenderKey,
        entity_manager::components::{AnimationAccessor, MeshAcessor},
        scene::SceneLoadLevel,
    },
};

pub mod asset_manager;
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

pub struct MeshRenderables {
    pub pnu_vertex_ranges: Option<Vec<Range<u32>>>,
    pub pnu_mesh_map: Vec<u32>,
    pub pnujw_vertex_ranges: Option<Vec<Range<u32>>>,
    pub pnujw_mesh_map: Vec<u32>,
    pub joint_transforms: Option<Vec<Mat4F32>>,
    pub joint_map: Vec<u32>,
    pub ibms: Option<Vec<Mat4F32>>,
    pub index_ranges: Option<Vec<Range<u32>>>,
    pub local_transforms: Vec<LocalTransform>,
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

pub enum UnloadedAssetData {
    Gltf(gltf::Gltf, BinarySource),
}
impl Debug for UnloadedAssetData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnloadedAssetData::Gltf(_, _) => write!(f, "Gltf Asset",),
        }
    }
}

impl UnloadedAssetData {
    fn load(self) -> Result<Box<dyn Asset>, ModelBuilderError> {
        match self {
            Self::Gltf(gltf, bin) => GltfAsset::load(gltf, bin),
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
#[derive(Clone, Debug)]
pub enum AssetResidency {
    Registered,
    PendingCPU,
    CPU(usize),
    PendingGPU(usize),
    GPU(GPUAllocationHandle, usize),
}
impl PartialEq<SceneLoadLevel> for AssetResidency {
    fn eq(&self, other: &SceneLoadLevel) -> bool {
        match self {
            AssetResidency::Registered | AssetResidency::PendingCPU => {
                if *other == SceneLoadLevel::NotLoaded {
                    return true;
                }
            }
            AssetResidency::CPU(_) | AssetResidency::PendingGPU(_) => {
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
            AssetResidency::Registered | AssetResidency::PendingCPU => match other {
                SceneLoadLevel::NotLoaded => return Some(Ordering::Equal),
                SceneLoadLevel::CPU | SceneLoadLevel::GPU => {
                    return Some(Ordering::Less);
                }
            },
            AssetResidency::CPU(_) | AssetResidency::PendingGPU(_) => match other {
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
    fn render_mesh_data<'a>(&self, mesh_accessor: &'a MeshAcessor) -> MeshRenderables;
}

pub trait ProvidesAnimationData: Asset {
    fn entity_animation<'a>(
        &self,
        animation_accessor: &AnimationAccessor,
        mesh_accessor: &MeshAcessor,
    ) -> EntityAnimationData;
}

pub struct LoadedAsset<'a> {
    pub asset: &'a Box<dyn Asset>,
    pub alloc_handle: GPUAllocationHandle,
}

impl<'a> Deref for LoadedAsset<'a> {
    type Target = &'a Box<dyn Asset>;

    fn deref(&self) -> &Self::Target {
        &self.asset
    }
}
