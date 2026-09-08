use crate::{
    app::GPUAssetUploadJob,
    asset_manager::{Asset, ProvidesMaterialData},
    util::types::GPUMaterialData,
};

pub struct MaterialAsset {
    pub base_color_factors: [f32; 4],
    pub roughness: f32,
    pub metallic: f32,
    pub tex_modifier: u32,
}

impl From<gltf::Material<'_>> for MaterialAsset {
    fn from(value: gltf::Material) -> Self {
        let pbr_mr = value.pbr_metallic_roughness();
        Self {
            base_color_factors: pbr_mr.base_color_factor(),
            roughness: pbr_mr.roughness_factor(),
            metallic: pbr_mr.metallic_factor(),
            tex_modifier: u32::MAX,
        }
    }
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
        let payload = GPUMaterialData {
            base_color_factors: self.base_color_factors,
            roughness: self.roughness,
            metallic: self.metallic,
            tex_modifier: self.tex_modifier,
            _pad: 0,
        };
        match job {
            GPUAssetUploadJob::ModelData { materials, .. } => {
                if let Some(material_payloads) = materials {
                    material_payloads.push(payload);
                } else {
                    materials.insert(vec![payload]);
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
                tex_modifier: self.tex_modifier,
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
