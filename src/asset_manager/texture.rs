use std::{io::Cursor, path::PathBuf};

use image::ImageReader;

use crate::{
    asset_manager::{BinaryData, gltf_asset::GltfLoadError},
    util::types::GPUTextureData,
};

pub fn load_texture_from_file(path: &PathBuf) -> Result<GPUTextureData, GltfLoadError> {
    let data = std::fs::read(path).map_err(|e| GltfLoadError::IOErr(e.kind()))?;
    let image = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .expect("invalid image type")
        .decode()
        .expect("failed to decode image");

    Ok(GPUTextureData {
        srgb: false,
        pixels: image.to_rgba8().into_raw().into(),
        height: image.height(),
        width: image.width(),
    })
}

pub fn decode_embedded(
    gltf: &gltf::Gltf,
    bin: &BinaryData,
    idx: usize,
) -> Result<GPUTextureData, GltfLoadError> {
    let texture = gltf.textures().nth(idx).ok_or(GltfLoadError::BadFile(
        "cannot find texture on the gltf file".to_string(),
    ))?;
    let gltf::image::Source::View { view, mime_type } = texture.source().source() else {
        panic!("texture source does not align with textures in gltf file");
    };

    let offset = view.offset() + bin.buffer_offsets[view.buffer().index()];
    let data = &bin.data[offset..(offset + view.length())];
    let image = ImageReader::new(Cursor::new(data))
        .decode()
        .expect("image read failure");
    Ok(GPUTextureData {
        height: image.height(),
        width: image.width(),
        srgb: false,
        pixels: image.to_rgba8().into_raw().into(),
    })
}
