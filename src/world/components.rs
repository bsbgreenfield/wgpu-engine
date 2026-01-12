use crate::{
    asset_manager::asset_manager::{Asset, AssetHandle},
    util::types::Mat4F32,
};

pub struct MeshCollectionComponent {
    local_transforms: Vec<Mat4F32>,
    asset_handle: AssetHandle,
}

pub(super) trait ExtractComponents {
    type Output;

    fn extract(asset: &AssetHandle) -> Self::Output;
}
