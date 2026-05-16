use std::{
    fmt::{Debug, Display},
    hash::Hash,
    path::PathBuf,
    sync::Arc,
};

use crate::{
    animation::animation::{AnimationChannels, AnimationSampler},
    app::GPUAssetUploadJob,
    asset_manager::{
        Asset, AssetHandle, AssetLoadError, ModelBuilderError, gltf_asset::mesh::Mesh,
    },
    util::types::{PNUJWVertex, PNUVertex, VIndex},
};
mod animation;
mod build;
mod loader;
mod mesh;
mod upload;
mod util;

#[allow(unused)]
#[derive(Clone)]
pub enum BinarySource {
    BinFile(PathBuf),
    GLB(PathBuf),
    GLTFBuffers(PathBuf),
    Undefined,
}

impl Asset for GltfAsset {
    fn new(dir_name: &str) -> Result<super::UnloadedAssetData, AssetLoadError>
    where
        Self: Sized,
    {
        let (gltf, bin) =
            crate::asset_manager::gltf_asset::loader::load_gltf_from_resource(dir_name)?;
        Ok(super::UnloadedAssetData::Gltf(gltf, bin))
    }

    fn get_upload_job(
        &self,
        asset_handle: AssetHandle,
    ) -> Result<GPUAssetUploadJob<'_>, AssetLoadError> {
        GPUAssetUploadJob::new(
            asset_handle,
            Some(&self.pnu_vertices[..]),
            Some(&self.pnujw_vertices[..]),
            self.indices.as_deref(),
        )
    }

    fn as_mesh_provider(&self) -> Option<&dyn super::ProvidesMeshData> {
        Some(self)
    }

    fn as_animation_provider(&self) -> Option<&dyn super::ProvidesAnimationData> {
        Some(self)
    }
}

pub struct GltfAnimation {
    pub samplers: Vec<AnimationSampler>,
    pub channels: AnimationChannels,
    pub root_nodes: Vec<Arc<GltfNode>>,
}
#[derive(Debug)]
pub(crate) enum NodeTransforms {
    Translation(cgmath::Vector3<f32>),
    Rotation(cgmath::Quaternion<f32>),
    Scale(cgmath::Vector3<f32>),
}

impl NodeTransforms {
    pub fn to_matrix(components: &[NodeTransforms; 3]) -> cgmath::Matrix4<f32> {
        let mut t = cgmath::Matrix4::from_translation(cgmath::Vector3::new(0.0, 0.0, 0.0));
        let mut r = cgmath::Matrix4::from(cgmath::Quaternion::new(1.0, 0.0, 0.0, 0.0));
        let mut s = cgmath::Matrix4::from_scale(1.0);
        for component in components {
            match component {
                NodeTransforms::Translation(v) => t = cgmath::Matrix4::from_translation(*v),
                NodeTransforms::Rotation(q) => r = cgmath::Matrix4::from(*q),
                NodeTransforms::Scale(v) => {
                    s = cgmath::Matrix4::from_nonuniform_scale(v.x, v.y, v.z)
                }
            }
        }
        t * r * s
    }
}

#[derive(Debug)]
pub enum NodeType {
    Mesh(usize),
    Joint((u32, u32)),
    Node,
}

#[derive(Debug)]
pub(crate) struct GltfNode {
    node_type: NodeType,
    node_id: usize,
    skin_idx: Option<usize>,
    children: Vec<GltfNode>,
    transform_components: [NodeTransforms; 3],
}

#[cfg(test)]
impl GltfNode {
    pub fn mock(
        node_type: NodeType,
        id: usize,
        skin_idx: Option<usize>,
        children: Vec<GltfNode>,
        transforms: [NodeTransforms; 3],
    ) -> Self {
        Self {
            skin_idx,
            node_type,
            node_id: id,
            children,
            transform_components: transforms,
        }
    }
}

impl PartialEq for GltfNode {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
    }
}

impl Eq for GltfNode {}

impl Hash for GltfNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node_id.hash(state);
    }
}

pub struct GltfAsset {
    node_tree: Vec<Arc<GltfNode>>,
    meshes: Vec<Mesh>,
    pnujw_vertices: Vec<PNUJWVertex>,
    pnu_vertices: Vec<PNUVertex>,
    indices: Option<Vec<VIndex>>,
    animations: Vec<Arc<GltfAnimation>>,
    skins: Vec<Vec<usize>>,
}

#[derive(Debug)]
pub enum GltfValidationError {
    NoView,
    UnsupportedScheme,
}
#[derive(Debug)]
pub enum GltfLoadError {
    IOErr(std::io::ErrorKind),
    InvalidFileError,
    MultipleFileTypes,
    GltfNeedsBinFile,
    GltfPackageError(gltf::Error),
    BadFile(String),
    ModelBuilderError(Box<ModelBuilderError>),
    Unimplemented,
}

pub enum GltfAttributeType {
    Position,
    Normal,
    TexCoords,
    Index,
    Joints,
    Weights,
    IBMS,
    Times,
    RotationT,
    TranslationT,
    ScaleT,
}

impl GltfAttributeType {
    pub fn from_animation_channel(channel: &gltf::animation::Channel) -> Self {
        match channel.target().property() {
            gltf::animation::Property::Translation => Self::TranslationT,
            gltf::animation::Property::Rotation => Self::RotationT,
            gltf::animation::Property::Scale => Self::ScaleT,
            _ => panic!(),
        }
    }
}

impl Display for GltfLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOErr(err) => Display::fmt(err, f),
            Self::InvalidFileError => f.write_str("Gltf load failed due to an invald file type"),
            Self::MultipleFileTypes => f.write_str("Gltf load failed due to there being multiple file types to choose from in the provided asset source file"),
            Self::GltfNeedsBinFile => f.write_str("Gltf load failed due to a missing bin file for the associated gltf file"),
            Self::GltfPackageError(err) => Display::fmt(err, f),
            Self::BadFile(str) => f.write_str(str),
            Self::ModelBuilderError(_) => f.write_str("Gltf load failed internally"),
            Self::Unimplemented => f.write_str("This type of gltf loading has not been implemented"),
        }
    }
}
impl std::error::Error for GltfLoadError {}

impl From<ModelBuilderError> for GltfLoadError {
    fn from(value: ModelBuilderError) -> Self {
        Self::ModelBuilderError(Box::new(value))
    }
}

impl From<gltf::Error> for GltfLoadError {
    fn from(value: gltf::Error) -> Self {
        Self::GltfPackageError(value)
    }
}
