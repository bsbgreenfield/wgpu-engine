use std::any::{Any, TypeId};

use crate::{
    asset_manager::{
        self,
        asset_manager::{Asset, AssetHandle, AssetLoadError, AssetManager, LoadedAsset},
        model_builder::GltfModelBuilder,
    },
    util::types::Mat4F32,
};

pub struct MeshCollectionComponent {
    local_transforms: Vec<Mat4F32>,
}

impl MeshCollectionComponent {
    pub fn new(local_transforms: Vec<Mat4F32>) -> Self {
        Self { local_transforms }
    }
}

pub trait ExtractComponents {
    type Output;

    fn extract_from(
        asset_manager: &AssetManager,
        asset: &AssetHandle,
    ) -> Result<Self::Output, AssetLoadError>;
}

impl ExtractComponents for (MeshCollectionComponent,) {
    type Output = Vec<MeshCollectionComponent>;

    fn extract_from(
        asset_manager: &AssetManager,
        asset: &AssetHandle,
    ) -> Result<Self::Output, AssetLoadError> {
        let builder = asset_manager
            .get_builder(asset)
            .ok_or(AssetLoadError::AssetNotFound)?;
        let res: Vec<LoadedAsset> = builder.as_ref().get_components()?;
    }
}
