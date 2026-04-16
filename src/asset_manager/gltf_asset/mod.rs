use std::{any::TypeId, fmt::Display, ops::Range, path::PathBuf};

use crate::{
    asset_manager::{Asset, LoadedAsset, Mesh, ModelBuilderError, asset_manager::AssetResidency},
    util::types::{LocalTransform, ModelVertex, PNUJWVertex, PNUVertex, VIndex},
};

mod build;
mod load;
mod primitive;

#[allow(unused)]
#[derive(Clone)]
pub(in crate::asset_manager) enum BinarySource {
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
        let res = Self::load_gltf_from_resource(dir_name)?;
        Ok(Self {
            res_level: AssetResidency::Registered,
            gltf: res.0,
            bin: res.1,
        })
    }

    fn get_residency_level(&self) -> &AssetResidency {
        &self.res_level
    }

    fn set_residency_level(&mut self, level: AssetResidency) {
        self.res_level = level;
    }

    fn load_asset(
        &self,
        handle: super::AssetHandle,
    ) -> Result<super::LoadedAsset, super::AssetLoadError> {
        let load_result = Self::build_gltf(&self.gltf, &self.bin)?;

        Ok(LoadedAsset {
            gltf_mesh_data: load_result,
            handle,
        })
    }
}
#[derive(Debug)]
pub struct GltfLoadResult {
    pub pnujw_vertices: Vec<PNUJWVertex>,
    pub pnu_vertices: Vec<PNUVertex>,
    pub indices: Option<Vec<VIndex>>,
    pub local_transforms: Vec<LocalTransform>,
    pub mesh_data: Vec<GltfMeshData>,
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

#[derive(Debug)]
pub enum GltfValidationError {
    NoView,
}
#[derive(Debug)]
pub struct GltfMeshData {
    meshes: Vec<Mesh>,
}
impl LoadedAsset {
    pub fn mesh_ids_and_alloc_ranges_of<V: ModelVertex>(
        &self,
    ) -> Option<(Vec<u32>, Vec<Range<u32>>, Option<Vec<Range<u32>>>)> {
        if TypeId::of::<V>() == TypeId::of::<PNUVertex>()
            && self.gltf_mesh_data.pnu_vertices.is_empty()
        {
            return None;
        } else if TypeId::of::<V>() == TypeId::of::<PNUJWVertex>()
            && self.gltf_mesh_data.pnujw_vertices.is_empty()
        {
            return None;
        }
        let mut mesh_ids = Vec::<u32>::new();
        let mut primitive_ranges = Vec::<Range<u32>>::new();
        let mut maybe_index_ranges = if self.gltf_mesh_data.indices.is_some() {
            Some(Vec::<Range<u32>>::new())
        } else {
            None
        };
        for mesh_data in self.gltf_mesh_data.mesh_data.iter() {
            // find all meshes which contain primitives of the correct type
            let filtered_meshes = mesh_data.meshes.iter().filter(|m| {
                m.primitives
                    .iter()
                    .any(|p| p.vertex_type == TypeId::of::<V>())
            });
            for filtered_mesh in filtered_meshes {
                for candidate_primitive in filtered_mesh.primitives.iter() {
                    if candidate_primitive.vertex_type == TypeId::of::<V>() {
                        mesh_ids.push(filtered_mesh.id);
                        // ADD PRIMITIVE COUNT FOR MODEL IF NECESSARY HERE
                        primitive_ranges.push(candidate_primitive.vertices.clone());
                        if let Some(index_ranges) = maybe_index_ranges.as_mut() {
                            index_ranges.push(candidate_primitive.indices.clone().expect("this primtive belongs to a models with defined indices, but it itself does not have any indicices specified"));
                        }
                    }
                }
            }
        }
        Some((mesh_ids, primitive_ranges, maybe_index_ranges))
    }
}
