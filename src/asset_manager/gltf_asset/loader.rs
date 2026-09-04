use std::{
    error::Error,
    fs::{DirEntry, ReadDir, read_dir},
    io::Cursor,
    path::PathBuf,
};

use base64::Engine;
use gltf::Gltf;
use image::DynamicImage;

use crate::asset_manager::{
    BinaryData,
    gltf_asset::{AssetSources, BinarySource, GltfLoadError, TextureSource},
};

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
        "base64" => base64_decode(encoded_data)?,
        other => return Err(format!("Unsupported encoding: {}", other).into()),
    };

    Ok(decoded)
}

pub(super) fn load_gltf_from_resource(
    dir_name: &str,
) -> Result<(gltf::Gltf, AssetSources), GltfLoadError> {
    let dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("res")
        .join(dir_name);
    if !dir_path.is_dir() {
        return Err(GltfLoadError::IOErr(std::io::ErrorKind::NotFound));
    }

    let mut dot_gltf: Option<PathBuf> = None;
    let mut dot_glb: Option<PathBuf> = None;
    let mut dot_bin: Vec<PathBuf> = vec![];
    let entries: ReadDir = read_dir(&dir_path).map_err(|e| GltfLoadError::IOErr(e.kind()))?;

    for maybe_entry in entries {
        let entry: DirEntry = maybe_entry.map_err(|_| GltfLoadError::InvalidFileError)?;
        match entry.path().extension().unwrap().to_str().unwrap() {
            "gltf" => dot_gltf = Some(entry.path()),
            "bin" => dot_bin.push(entry.path()),
            "glb" => dot_glb = Some(entry.path()),
            _ => {}
        }
    }

    if dot_glb.is_some() && dot_gltf.is_some() {
        return Err(GltfLoadError::MultipleFileTypes);
    }

    if let Some(gltf_file) = dot_gltf {
        let gltf_res: gltf::Gltf =
            Gltf::open(&gltf_file).map_err(|e| GltfLoadError::GltfPackageError(e))?;

        let binary_buffer_sources = get_binary_buffer_sources(&gltf_res, dir_name)?;
        let texture_sources = get_textures(&gltf_res, &dot_bin)?;

        return Ok((
            gltf_res,
            AssetSources {
                binary_sources: binary_buffer_sources,
                textures: texture_sources,
            },
        ));
    } else {
        return Err(GltfLoadError::Unimplemented);
    }
}

fn get_binary_buffer_sources(
    gltf: &gltf::Gltf,
    gltf_dir: &str,
) -> Result<Vec<BinarySource>, GltfLoadError> {
    let mut res: Vec<BinarySource> = Vec::with_capacity(gltf.buffers().len());
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("res")
        .join(gltf_dir);
    for buffer in gltf.buffers() {
        match buffer.source() {
            gltf::buffer::Source::Bin => {
                res.push(BinarySource::GLTFBuffers);
            }
            gltf::buffer::Source::Uri(uri) => {
                res.push(BinarySource::BinFile(base_path.join(PathBuf::from(uri))));
            }
        }
    }
    return Ok(res);
}

fn get_textures(
    gltf_res: &gltf::Gltf,
    bin_files: &[PathBuf],
) -> Result<Vec<TextureSource>, GltfLoadError> {
    let mut texture_sources = Vec::new();
    for tex in gltf_res.textures() {
        let texture_source: Result<TextureSource, GltfLoadError> = match tex.source().source() {
            gltf::image::Source::View { view, mime_type } => match view.buffer().source() {
                gltf::buffer::Source::Bin => {
                    Ok(TextureSource::BinarySource(BinarySource::GLTFBuffers))
                }
                gltf::buffer::Source::Uri(file_path) => {
                    let bin_match = bin_files
                        .iter()
                        .find(|bin_file_path| *bin_file_path == file_path)
                        .ok_or(GltfLoadError::InvalidFileError)?;
                    Ok(TextureSource::BinarySource(BinarySource::BinFile(
                        bin_match.into(),
                    )))
                }
            },
            gltf::image::Source::Uri { uri, mime_type } => {
                // TODO: allow inline uris
                let dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("res")
                    .join("textures")
                    .join(uri);
                if !dir_path.is_file() {
                    return Err(GltfLoadError::InvalidFileError);
                }
                Ok(TextureSource::ExternalFile(dir_path))
            }
        };
        texture_sources.push(texture_source?);
    }
    Ok(texture_sources)
}

pub(super) fn load_binary_data_from_source(
    gltf: &gltf::Gltf,
    sources: &AssetSources,
) -> Result<BinaryData, GltfLoadError> {
    let mut res: Vec<u8> = Vec::new();
    let mut buffer_offsets: Vec<usize> = Vec::new();
    buffer_offsets.push(0);
    for bin_source in sources.binary_sources.iter() {
        match bin_source {
            BinarySource::BinFile(path) => {
                println!("{:?}", path);
                let data = std::fs::read(path).map_err(|e| GltfLoadError::IOErr(e.kind()))?;
                buffer_offsets.push(data.len());
                res.extend(data);
            }
            BinarySource::GLTFBuffers => {
                for bin_buffer in gltf.buffers() {
                    let gltf::buffer::Source::Uri(uri) = bin_buffer.source() else {
                        continue;
                    };
                    let data = decode_gltf_data_uri(uri)
                        .map_err(|_| GltfLoadError::BadFile("provided gltf file".to_string()))?;
                    buffer_offsets.push(data.len());
                    res.extend(data);
                }
            }
            BinarySource::GLB(_) => todo!("havent implemented glbs yet"),
            BinarySource::Undefined => panic!("undefined source"),
        }
    }
    Ok(BinaryData {
        buffer_offsets,
        data: res,
    })
}
