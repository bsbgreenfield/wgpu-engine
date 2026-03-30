use crate::{
    asset_manager::{
        asset_manager::AssetResidency,
        gltf_assets::{
            gltf_loader::{GltfLoadError, loader::BinarySource},
            model_builder_new::GltfMeshData,
            primitive::GltfValidationError,
        },
    },
    util::types::{LocalTransform, PNUJWVertex, PNUVertex},
};

pub(super) mod gltf_loader;
pub mod mesh;
pub mod model_builder_new;
mod primitive;
#[derive(Debug)]
pub enum ModelBuilderError {
    NodeNotFound(usize),
    GLTFUndefined,
    GLTFLoadError(GltfLoadError),
    MeshNotFound(usize),
    ValidationError(GltfValidationError),
    BinarySourceNotFound,
    IndexRangeError,
}

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
    pub indices: Vec<u16>,
    pub local_transforms: Vec<LocalTransform>,
    pub mesh_data: Vec<GltfMeshData>,
}
