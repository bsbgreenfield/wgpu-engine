use std::{fmt::Display, sync::Arc};

use crate::{
    asset_manager::{
        AssetHandle,
        asset_manager::AssetManager,
        gltf_asset::{GltfMaterial, GltfTexture},
        material::MaterialTexture,
    },
    renderer::{GPUAllocationHandle, RenderError, RenderUpdateError},
    util::types::{AssetIndices, GPUMaterialData, GPUTextureData, PNUJWVertex, PNUVertex},
    world::WorldUpdateError,
};

pub mod app;
pub mod app_config;
pub mod app_state;

#[derive(Clone)]
pub struct EmbeddedMaterialPayload {
    pub material: GPUMaterialData,
    pub texture: Option<AssetHandle>,
}
#[derive(Clone, Debug)]
pub enum GPUTextureBinding {
    None,
    Embedded(Arc<GPUTextureData>),
    Resolved(GPUAllocationHandle),
}

#[derive(Clone, Debug, Default)]
pub struct MaterialPaletteJob {
    pub records: Vec<GPUMaterialData>,
    pub textures: Vec<GPUTextureBinding>,
}

impl MaterialPaletteJob {
    pub fn from_gltf(materials: &[GltfMaterial], asset_manager: &AssetManager) -> Self {
        let mut records: Vec<GPUMaterialData> = Vec::with_capacity(materials.len());
        let mut textures: Vec<GPUTextureBinding> = Vec::with_capacity(materials.len());
        for material in materials {
            let pbr = &material.pbr_metallic_roughness;
            let material_data = GPUMaterialData {
                base_color_factors: pbr.base_color_factor,
                roughness: pbr.roughness,
                metallic: pbr.metallicness,
                tex_mod: 0,
                _pad: 0,
            };
            let texture = match &pbr.texture {
                None => GPUTextureBinding::None,
                Some(GltfTexture::External(handle)) => GPUTextureBinding::Resolved(
                    asset_manager
                        .alloc_handle_of(handle)
                        .expect("this should already be gpu resident"),
                ),
                Some(GltfTexture::Embedded(image)) => {
                    GPUTextureBinding::Embedded(Arc::new(GPUTextureData {
                        height: image.height(),
                        width: image.width(),
                        srgb: false,
                        pixels: image.to_rgba8().into_raw().into(),
                    }))
                }
            };
            records.push(material_data);
            textures.push(texture);
        }
        Self { records, textures }
    }
}

#[derive(Clone)]
pub enum GPUAssetUploadJob {
    ModelData {
        asset_handle: AssetHandle,
        pnu_vertices: Option<Arc<[PNUVertex]>>,
        pnujw_vertices: Option<Arc<[PNUJWVertex]>>,
        indices: Option<AssetIndices>,
        embedded_materials: MaterialPaletteJob,
    },
    MaterialData {
        asset_handle: AssetHandle,
        material_data: GPUMaterialData,
        texture: Option<MaterialTexture>,
    },
    TextureData {
        asset_handle: AssetHandle,
        data: GPUTextureData,
    },
}

impl GPUAssetUploadJob {
    pub fn material_upload_from_gltf(
        asset_handle: &AssetHandle,
        material: &GltfMaterial,
        asset_manager: &AssetManager,
    ) -> Self {
        let texture = material
            .pbr_metallic_roughness
            .texture
            .as_ref()
            .map(|t| match t {
                crate::asset_manager::gltf_asset::GltfTexture::External(tex_asset_handle) => {
                    let texture_alloc = asset_manager.alloc_handle_of(tex_asset_handle)
                        .expect("this texture has not been gpu uploaded, and so the material is not ready for upload");
                    MaterialTexture::Resolved(texture_alloc)
                }
                crate::asset_manager::gltf_asset::GltfTexture::Embedded(dynamic_image) => {
                    MaterialTexture::Embedded(
                        GPUTextureData {
                            height: dynamic_image.height(),
                            width: dynamic_image.width(),
                            srgb: false,
                            pixels: dynamic_image.to_rgba8().into_raw().into(),
                        }
                        .into(),
                    )
                }
            });
        Self::MaterialData {
            asset_handle: *asset_handle,
            material_data: GPUMaterialData {
                base_color_factors: material.pbr_metallic_roughness.base_color_factor.clone(),
                roughness: material.pbr_metallic_roughness.roughness.clone(),
                metallic: material.pbr_metallic_roughness.metallicness.clone(),
                tex_mod: 0,
                _pad: 0,
            },
            texture: texture,
        }
    }
}

#[allow(unused)]
#[derive(Debug)]
pub enum FrameError {
    UpdateError(WorldUpdateError),
    SurfaceError(wgpu::CreateSurfaceError),
    RenderUpdateError(RenderUpdateError),
    RenderError(RenderError),
}

impl Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UpdateError(err) => err.fmt(f),
            Self::SurfaceError(err) => err.fmt(f),
            Self::RenderUpdateError(err) => err.fmt(f),
            Self::RenderError(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<WorldUpdateError> for FrameError {
    fn from(value: WorldUpdateError) -> Self {
        FrameError::UpdateError(value)
    }
}

impl From<RenderUpdateError> for FrameError {
    fn from(value: RenderUpdateError) -> Self {
        FrameError::RenderUpdateError(value)
    }
}

impl From<RenderError> for FrameError {
    fn from(value: RenderError) -> Self {
        FrameError::RenderError(value)
    }
}
