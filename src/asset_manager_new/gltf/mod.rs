use std::{any::TypeId, fmt::Display, path::PathBuf};

use crate::{
    app::GPUUploadJob,
    asset_manager_new::{
        Asset, AssetHandle, AssetLoadError, AssetResidency, LoadedAsset, ModelBuilderError,
        gltf::mesh::Mesh,
    },
    util::types::{Mat4F32, PNUJWVertex, PNUVertex, VIndex},
};
mod build;
mod loader;
mod mesh;

#[allow(unused)]
#[derive(Clone)]
pub(in crate::asset_manager_new) enum BinarySource {
    BinFile(PathBuf),
    GLB(PathBuf),
    GLTFBuffers(PathBuf),
    Undefined,
}
pub struct GltfAsset {
    gltf: gltf::Gltf,
    bin: BinarySource,
    res_level: AssetResidency,
}

impl Asset for GltfAsset {
    fn new(dir_name: &str) -> Result<Self, super::AssetLoadError>
    where
        Self: Sized,
    {
        let res = super::gltf::loader::load_gltf_from_resource(dir_name)?;
        Ok(Self {
            gltf: res.0,
            bin: res.1,
            res_level: AssetResidency::Registered,
        })
    }
}
struct GltfNode {
    mesh_id: Option<usize>,
    node_id: usize,
    children: Vec<GltfNode>,
    transform: Mat4F32,
}

pub(super) struct LoadedGltfAsset {
    node_tree: Vec<GltfNode>,
    meshes: Vec<Mesh>,
    pnujw_vertices: Vec<PNUJWVertex>,
    pnu_vertices: Vec<PNUVertex>,
    indices: Option<Vec<VIndex>>,
}

impl LoadedAsset for LoadedGltfAsset {
    fn upload_job<'a>(
        &'a self,
        asset_handle: &'a AssetHandle,
    ) -> Result<GPUUploadJob<'a>, AssetLoadError> {
        GPUUploadJob::new(
            asset_handle,
            Some(&self.pnu_vertices[..]),
            Some(&self.pnujw_vertices[..]),
            self.indices.as_deref(),
        )
    }
    fn get_renderables(&self) -> Option<crate::world::entity_manager::Renderables> {
        let mut pnu_ranges = Vec::new();
        let mut pnujw_ranges = Vec::new();

        for mesh in self.meshes.iter() {
            for primitive in mesh.primitives.iter() {
                if primitive.vertex_type == TypeId::of::<PNUVertex>() {
                    pnu_ranges.push(primitive.vertices.clone());
                } else if primitive.vertex_type == TypeId::of::<PNUVertex>() {
                    pnujw_ranges.push(primitive.vertices.clone());
                } else {
                    panic!("vertex type not specified {:?}", primitive.vertex_type);
                }

                todo!("INDEX RANGES")
            }
        }

        todo!()
    }
}

#[derive(Debug)]
pub(super) enum GltfValidationError {
    NoView,
    UnsupportedScheme,
}
#[derive(Debug)]
pub(super) enum GltfLoadError {
    IOErr(std::io::ErrorKind),
    InvalidFileError,
    MultipleFileTypes,
    GltfNeedsBinFile,
    GltfPackageError(gltf::Error),
    BadFile(String),
    ModelBuilderError(Box<ModelBuilderError>),
    Unimplemented,
}

impl Display for GltfLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOErr(err) => err.fmt(f),
            Self::InvalidFileError => f.write_str("Gltf load failed due to an invald file type"),
            Self::MultipleFileTypes => f.write_str("Gltf load failed due to there being multiple file types to choose from in the provided asset source file"),
            Self::GltfNeedsBinFile => f.write_str("Gltf load failed due to a missing bin file for the associated gltf file"),
            Self::GltfPackageError(err) => err.fmt(f),
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
