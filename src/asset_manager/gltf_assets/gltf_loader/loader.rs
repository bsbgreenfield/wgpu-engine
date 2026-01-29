pub struct GltfLoader;

#[derive(Clone)]
pub enum BinarySource {
    BinFile(PathBuf),
    GLB(PathBuf),
    GLTFBuffers(PathBuf),
    Undefined,
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

impl From<ModelBuilderError> for GltfLoadError {
    fn from(value: ModelBuilderError) -> Self {
        Self::ModelBuilderError(Box::new(value))
    }
}

use std::{
    error::Error,
    fs::{self, DirEntry, ReadDir},
    path::PathBuf,
};

use base64::Engine;
use gltf::Gltf;

use crate::asset_manager::gltf_assets::model_builder::ModelBuilderError;

impl GltfLoader {
    fn base64_decode(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        use base64::prelude::BASE64_STANDARD;
        // Uses standard lib base64 via experimental feature or stable crate if you choose
        let decoded = BASE64_STANDARD.decode(input)?; // Requires base64 crate
        Ok(decoded)
    }

    fn decode_gltf_data_uri(uri: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        // Step 1: Check prefix
        const PREFIX: &str = "data:application/gltf-buffer;";
        if !uri.starts_with(PREFIX) {
            return Err("URI does not start with expected prefix".into());
        }

        // Step 2: Split metadata and encoded data
        let comma_index = uri.find(',').ok_or("No comma found in URI")?;
        let (meta, encoded_data) = uri[PREFIX.len()..].split_at(comma_index - PREFIX.len());
        let encoded_data = &encoded_data[1..]; // Skip the comma

        // Step 3: Match encoding and decode
        let decoded = match meta.trim() {
            "base64" => Self::base64_decode(encoded_data)?,
            other => return Err(format!("Unsupported encoding: {}", other).into()),
        };

        Ok(decoded)
    }

    pub fn load_binary_data_from_source(source: &BinarySource) -> Result<Vec<u8>, GltfLoadError> {
        match source {
            BinarySource::BinFile(path) => {
                return std::fs::read(path).map_err(|e| GltfLoadError::IOErr(e.kind()));
            }
            BinarySource::GLTFBuffers(path) => {
                let gltf =
                    gltf::Gltf::open(&path).map_err(|e| GltfLoadError::GltfPackageError(e))?;
                let mut bin_data = Vec::<u8>::new();
                for buffer in gltf.buffers() {
                    let data = match buffer.source() {
                        gltf::buffer::Source::Bin => return Err(GltfLoadError::GltfNeedsBinFile),
                        gltf::buffer::Source::Uri(uri) => GltfLoader::decode_gltf_data_uri(uri)
                            .map_err(|_| {
                                GltfLoadError::BadFile(
                                    path.to_str().unwrap_or("Provided GLTF File").to_string(),
                                )
                            }),
                    };
                    bin_data.extend(data?);
                }
                return Ok(bin_data);
            }
            BinarySource::GLB(_) => todo!("haven't implemented glbs yet"),
            BinarySource::Undefined => return Err(GltfLoadError::GltfNeedsBinFile),
        }
    }

    fn load_from_separate_data_files(
        gltf_file: &PathBuf,
        bin_file: PathBuf,
    ) -> Result<(gltf::Gltf, BinarySource), GltfLoadError> {
        let gtlf_res: gltf::Gltf =
            Gltf::open(gltf_file).map_err(|e| GltfLoadError::GltfPackageError(e))?;

        Ok((gtlf_res, BinarySource::BinFile(bin_file)))
    }

    fn load_from_single_gltf_file(
        gltf_file: PathBuf,
    ) -> Result<(gltf::Gltf, BinarySource), GltfLoadError> {
        let gltf_res: gltf::Gltf =
            Gltf::open(&gltf_file).map_err(|e| GltfLoadError::GltfPackageError(e))?;

        Ok((gltf_res, BinarySource::GLTFBuffers(gltf_file)))
    }

    pub fn load_gltf_from_resource(
        dir_name: &str,
    ) -> Result<(gltf::Gltf, BinarySource), GltfLoadError> {
        let dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("res")
            .join(dir_name);
        if !dir_path.is_dir() {
            return Err(GltfLoadError::IOErr(std::io::ErrorKind::NotFound));
        }

        let mut dot_gltf: Option<PathBuf> = None;
        let mut dot_glb: Option<PathBuf> = None;
        let mut dot_bin: Option<PathBuf> = None;
        let entries: ReadDir =
            fs::read_dir(&dir_path).map_err(|e| GltfLoadError::IOErr(e.kind()))?;

        for maybe_entry in entries {
            let entry: DirEntry = maybe_entry.map_err(|_| GltfLoadError::InvalidFileError)?;
            match entry.path().extension().unwrap().to_str().unwrap() {
                "gltf" => dot_gltf = Some(entry.path()),
                "bin" => dot_bin = Some(entry.path()),
                "glb" => dot_glb = Some(entry.path()),
                _ => {}
            }
        }

        if dot_glb.is_some() && dot_gltf.is_some() {
            return Err(GltfLoadError::MultipleFileTypes);
        }
        if dot_gltf.is_some() && dot_bin.is_some() {
            let result = Self::load_from_separate_data_files(&dot_gltf.unwrap(), dot_bin.unwrap())?;
            return Ok(result);
        } else if dot_gltf.is_some() && dot_bin.is_none() {
            let result = Self::load_from_single_gltf_file(dot_gltf.unwrap())?;
            return Ok(result);
        } else {
            return Err(GltfLoadError::Unimplemented);
        }
    }
}
