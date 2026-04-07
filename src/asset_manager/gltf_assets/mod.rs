use std::fmt::{Debug, Display};

use crate::{
    asset_manager::{
        asset_manager::AssetResidency,
        gltf_assets::{
            gltf_loader::BinarySource, model_builder_new::GltfMeshData,
            primitive::GltfValidationError,
        },
    },
    util::types::{LocalTransform, PNUJWVertex, PNUVertex, VIndex},
};

pub(super) mod gltf_loader;
pub mod mesh;
pub mod model_builder_new;
mod primitive;
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
pub struct GltfAsset {
    gltf: gltf::Gltf,
    bin: BinarySource,
    res_level: AssetResidency,
}

#[derive(Debug)]
pub struct GltfLoadResult {
    pub pnujw_vertices: Vec<PNUJWVertex>,
    pub pnu_vertices: Vec<PNUVertex>,
    pub indices: Option<Vec<VIndex>>,
    pub local_transforms: Vec<LocalTransform>,
    pub mesh_data: Vec<GltfMeshData>,
}
