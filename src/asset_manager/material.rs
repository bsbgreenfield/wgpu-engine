use std::sync::Arc;

use crate::{
    app::GPUAssetUploadJob,
    asset_manager::{
        Asset, AssetHandle, ProvidesMaterialData, asset_manager::AssetManager,
        gltf_asset::GltfMaterial,
    },
    renderer::GPUAllocationHandle,
    util::types::{GPUMaterialData, GPUTextureData},
    world::entity_manager::components::ComponentAccessor,
};

#[derive(Clone)]
pub enum MaterialTexture {
    External(AssetHandle),
    Embedded(Arc<GPUTextureData>),
    Resolved(GPUAllocationHandle),
}

#[derive(Clone)]
pub struct MaterialAsset {
    pub base_color_factors: [f32; 4],
    pub roughness: f32,
    pub metallic: f32,
    pub texture: Option<MaterialTexture>,
}

impl From<&GltfMaterial> for MaterialAsset {
    fn from(value: &GltfMaterial) -> Self {
        let pbr = value.pbr_metallic_roughness.clone();
        Self {
            base_color_factors: pbr.base_color_factor,
            roughness: pbr.roughness,
            metallic: pbr.metallicness,
            texture: pbr.texture.map(|gltf_texture| match gltf_texture {
                super::gltf_asset::GltfTexture::External(asset_handle) => {
                    MaterialTexture::External(asset_handle)
                }
                super::gltf_asset::GltfTexture::Embedded(dynamic_image) => {
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
            }),
        }
    }
}

impl ProvidesMaterialData for MaterialAsset {
    fn material_palette<'a>(&self, _material_accessor: &'a ComponentAccessor) -> Vec<u32> {
        todo!()
    }
}
#[allow(unused)]
impl Asset for MaterialAsset {
    fn get_upload_job(
        &self,
        asset_handle: super::AssetHandle,
        asset_manager: &AssetManager,
    ) -> Result<crate::app::GPUAssetUploadJob, super::AssetLoadError> {
        Ok(GPUAssetUploadJob::MaterialData {
            asset_handle,
            material_data: GPUMaterialData {
                base_color_factors: self.base_color_factors,
                roughness: self.roughness,
                metallic: self.metallic,
                tex_mod: 0,
                _pad: 0,
            },
            texture: todo!(),
        })
    }

    fn as_mesh_provider(&self) -> Option<&dyn super::ProvidesMeshData> {
        None
    }

    fn as_animation_provider(&self) -> Option<&dyn super::ProvidesAnimationData> {
        None
    }

    fn as_materials_provider(&self) -> Option<&dyn super::ProvidesMaterialData> {
        Some(self)
    }
    fn as_texture_provider(&self) -> Option<&dyn super::ProvidesTextureData> {
        //TODO: can proide a texture?
        None
    }
}
