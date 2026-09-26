use std::{cell::Cell, collections::HashMap, error::Error, io::Cursor, path::PathBuf, sync::Arc};

use image::{DynamicImage, ImageReader};

use crate::{
    app::GPUAssetUploadJob,
    asset_manager::{
        Asset, AssetHandle, AssetSource, BinaryData, ModelBuilderError, ProvidesTextureData,
        asset_manager::AssetManager, gltf_asset::GltfLoadError,
    },
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

pub(super) fn load_image_from_file(path: &PathBuf) -> Result<DynamicImage, Box<dyn Error>> {
    let data = std::fs::read(path)?;
    let image = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .expect("invalid image type")
        .decode()
        .expect("failed to decode image");
    Ok(image)
}

pub fn decode_embedded_parallel(
    gltf: &gltf::Gltf,
    bin: &BinaryData,
    indices: &[usize],
) -> Result<HashMap<usize, Arc<image::DynamicImage>>, ModelBuilderError> {
    if indices.is_empty() {
        return Ok(HashMap::new());
    }

    // number of threads to use is the min of available threads and textures to decode
    let thread_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(indices.len());
    // split the work into the available threads
    let chunk_size = indices.len().div_ceil(thread_count);

    let chunks = std::thread::scope(|scope| {
        indices
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&idx| (idx, decode_embedded(gltf, bin, idx)))
                        .collect::<Vec<_>>()
                })
            })
            .map(|handle| handle.join().expect("texture decode fail"))
            .collect::<Vec<_>>()
    });
    let mut res = HashMap::with_capacity(indices.len());
    for (idx, result) in chunks.into_iter().flatten() {
        let image = result.map_err(|e| ModelBuilderError::GltfLoadError(e))?;
        res.insert(idx, Arc::new(image));
    }

    Ok(res)
}

pub(super) fn decode_embedded(
    gltf: &gltf::Gltf,
    bin: &BinaryData,
    idx: usize,
) -> Result<DynamicImage, GltfLoadError> {
    let texture = gltf.textures().nth(idx).ok_or(GltfLoadError::BadFile(
        "cannot find texture on the gltf file".to_string(),
    ))?;
    let gltf::image::Source::View { view, mime_type: _ } = texture.source().source() else {
        panic!("texture source does not align with textures in gltf file");
    };

    let offset = view.offset() + bin.buffer_offsets[view.buffer().index()];
    let data = &bin.data[offset..(offset + view.length())];
    let image = ImageReader::new(Cursor::new(data))
        .decode()
        .expect("image read failure");
    Ok(image)
}

pub struct TextureAsset {
    data: Cell<Option<DynamicImage>>,
}

impl ProvidesTextureData for TextureAsset {
    fn texture_data(
        &self,
        _texture_accessor: &crate::world::entity_manager::components::ComponentAccessor,
    ) -> DynamicImage {
        self.data.take().unwrap()
    }
}

impl AssetSource for TextureAsset {
    fn new(dir_name: &str) -> Result<super::UnloadedAssetData, super::AssetLoadError>
    where
        Self: Sized,
    {
        let dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("res")
            .join("textures")
            .join(dir_name);
        return Ok(super::UnloadedAssetData::Texture(dir_path));
    }
}
impl Asset for TextureAsset {
    fn get_upload_job(
        &self,
        asset_handle: super::AssetHandle,
        _asset_manager: &AssetManager,
    ) -> Result<crate::app::GPUAssetUploadJob, super::AssetLoadError> {
        let image = self.data.take().unwrap();

        Ok(GPUAssetUploadJob::TextureData {
            asset_handle,
            data: Arc::new(GPUTextureData {
                height: image.height(),
                width: image.width(),
                srgb: false,
                pixels: image.to_rgba8().into_raw().into(),
            }),
        })
    }

    fn as_mesh_provider(&self) -> Option<&dyn super::ProvidesMeshData> {
        None
    }

    fn as_animation_provider(&self) -> Option<&dyn super::ProvidesAnimationData> {
        None
    }

    fn as_materials_provider(&self) -> Option<&dyn super::ProvidesMaterialData> {
        None
    }

    fn as_texture_provider(&self) -> Option<&dyn super::ProvidesTextureData> {
        Some(self)
    }
}
impl TextureAsset {
    pub fn load(path: &PathBuf) -> Result<Box<dyn Asset>, ModelBuilderError> {
        let data = load_image_from_file(path).expect("texture laod failed");
        Ok(Box::new(TextureAsset {
            data: Cell::new(Some(data)),
        }))
    }
    pub fn from_gltf_binary(gltf: &gltf::Gltf, bin: &BinaryData, idx: usize) -> Self {
        let image = decode_embedded(gltf, bin, idx).expect("fail to decode");
        Self {
            data: Cell::new(Some(image)),
        }
    }
}
