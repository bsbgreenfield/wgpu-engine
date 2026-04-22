use std::error::Error;

use base64::Engine;

use crate::asset_manager_new::{
    AssetLoadError,
    gltf::{BinarySource, GltfLoadError},
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
) -> Result<(gltf::Gltf, BinarySource), AssetLoadError> {
    todo!()
}
pub(super) fn load_binary_data_from_source(
    source: &BinarySource,
) -> Result<Vec<u8>, GltfLoadError> {
    match source {
        BinarySource::BinFile(path) => {
            return std::fs::read(path).map_err(|e| GltfLoadError::IOErr(e.kind()));
        }
        BinarySource::GLTFBuffers(path) => {
            let gltf = gltf::Gltf::open(&path).map_err(|e| GltfLoadError::GltfPackageError(e))?;
            let mut bin_data = Vec::<u8>::new();
            for buffer in gltf.buffers() {
                let data = match buffer.source() {
                    gltf::buffer::Source::Bin => return Err(GltfLoadError::GltfNeedsBinFile),
                    gltf::buffer::Source::Uri(uri) => decode_gltf_data_uri(uri).map_err(|_| {
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
