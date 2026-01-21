use crate::{
    asset_manager::{
        asset_manager::Asset, asset_manager::AssetLoadError,
        gltf_assets::model_builder::GltfBuilderRegistered,
    },
    world::components::MeshCollectionComponent,
};

pub struct GltfAsset;

impl Asset for GltfAsset {
    type Builder = GltfBuilderRegistered;
    type Components = (MeshCollectionComponent,);
    fn builder(dir_name: &str) -> Result<Self::Builder, AssetLoadError> {
        GltfBuilderRegistered::new(dir_name)
    }
}
