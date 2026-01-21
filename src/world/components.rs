use std::{
    any::{Any, TypeId},
    rc::Rc,
};

use crate::{
    asset_manager::asset_manager::{AssetHandle, AssetLoadError, AssetManager},
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
        asset_manager: &mut AssetManager,
        asset: &AssetHandle,
    ) -> Result<Self::Output, AssetLoadError>;
}

impl ExtractComponents for (MeshCollectionComponent,) {
    type Output = Vec<Rc<MeshCollectionComponent>>;

    // TODO: This is perhaps not ideal. If we want to extract a single mesh collection from an
    // asset we need to get and clone all of them.
    // question: when and how often do we need to extract components for entities?
    // question 2: how to create multiple entites from a single LoadedAsset?
    fn extract_from(
        asset_manager: &mut AssetManager,
        asset: &AssetHandle,
    ) -> Result<Self::Output, AssetLoadError> {
        let loaded_asset = asset_manager.get_components_for(asset)?;
        let mesh_collection_refs = loaded_asset
            .get(&TypeId::of::<MeshCollectionComponent>())
            .ok_or(AssetLoadError::ComponentNotFound)?;
        let mut res = Vec::new();
        for mc_ref in mesh_collection_refs {
            res.push(
                mc_ref
                    .downcast::<MeshCollectionComponent>()
                    .unwrap()
                    .clone(),
            );
        }

        Ok(res)
    }
}
