use crate::{
    app::GPUAssetUploadJob,
    asset_manager::{Asset, AssetHandle, ProvidesMaterialData, asset_manager::InternedAssetKey},
    util::types::GPUMaterialData,
};

#[derive(Clone)]
pub enum MaterialTextureKey {
    External(AssetHandle),
    Embedded(InternedAssetKey),
}

#[derive(Clone)]
pub struct MaterialAsset {
    pub base_color_factors: [f32; 4],
    pub roughness: f32,
    pub metallic: f32,
    pub texture: Option<MaterialTextureKey>,
}

impl ProvidesMaterialData for MaterialAsset {
    fn material_data<'a>(
        &self,
        material_accessor: &'a crate::world::entity_manager::components::ComponentAccessor,
    ) -> Vec<super::MaterialRenderables> {
        todo!()
    }
}
impl Asset for MaterialAsset {
    fn intern_payload(&self, job: &mut GPUAssetUploadJob) -> () {
        match job {
            GPUAssetUploadJob::ModelData {
                embedded_materials, ..
            } => {
                if let Some(material_payloads) = embedded_materials {
                    material_payloads.push(self.clone());
                } else {
                    embedded_materials.insert(vec![self.clone()]);
                }
            }
            GPUAssetUploadJob::MaterialData {
                asset_handle,
                material_data,
            } => todo!(),
            GPUAssetUploadJob::TextureData { .. } => todo!(),
        }
    }
    fn get_upload_job(
        &self,
        asset_handle: super::AssetHandle,
    ) -> Result<crate::app::GPUAssetUploadJob, super::AssetLoadError> {
        Ok(GPUAssetUploadJob::MaterialData {
            asset_handle,
            material_data: GPUMaterialData {
                base_color_factors: self.base_color_factors,
                roughness: self.roughness,
                metallic: self.metallic,
                tex_modifier: todo!(),
                _pad: 0,
            },
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
