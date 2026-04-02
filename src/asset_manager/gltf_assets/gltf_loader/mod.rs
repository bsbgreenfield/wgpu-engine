use std::{fmt::Display, path::PathBuf};

use crate::asset_manager::gltf_assets::ModelBuilderError;

pub(in crate::asset_manager) mod loader;
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

#[allow(unused)]
#[derive(Clone)]
pub(in crate::asset_manager) enum BinarySource {
    BinFile(PathBuf),
    GLB(PathBuf),
    GLTFBuffers(PathBuf),
    Undefined,
}
