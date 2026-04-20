use std::{
    any::TypeId,
    collections::HashMap,
    fmt::{Debug, Display},
    ops::Range,
};

use crate::{
    asset_manager::{
        asset_manager::AssetResidency,
        gltf_asset::{GltfLoadError, GltfLoadResult, GltfValidationError},
    },
    util::types::{LocalTransform, Mat4F32},
};

pub mod asset_manager;
pub(super) mod gltf_asset;
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
pub struct Mesh {
    pub id: u32,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug)]
pub struct Primitive {
    pub vertex_type: TypeId,
    pub vertices: Range<u32>,
    pub indices: Option<Range<u32>>,
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
#[allow(unused)]
struct ModelJointData {
    joint_ids: Vec<usize>,
    joint_pose_transforms: Mat4F32,
    node_to_joint_id_map: HashMap<usize, usize>,
}
#[allow(unused)]
struct ModelData {
    id: usize,
    mesh_ids: Vec<usize>,
    joint_data: Option<ModelJointData>,
    local_transforms: Vec<LocalTransform>,
}

impl ModelData {
    fn new(id: usize, mesh_count: usize) -> Self {
        Self {
            id,
            mesh_ids: Vec::with_capacity(mesh_count),
            joint_data: None,
            local_transforms: Vec::with_capacity(mesh_count),
        }
    }

    fn get_local_transform_map(
        mesh_ids: Vec<usize>,
        local_transforms: Vec<LocalTransform>,
    ) -> HashMap<usize, LocalTransform> {
        let mut map = HashMap::new();
        for (id, lt) in mesh_ids.iter().zip(local_transforms) {
            map.insert(*id, lt);
        }
        map
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
