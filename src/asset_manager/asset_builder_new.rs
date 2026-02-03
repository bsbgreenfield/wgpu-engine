use std::{collections::HashMap, marker::PhantomData};

use crate::{
    asset_manager::{
        Asset,
        asset_manager::{AssetHandle, AssetResidencyLevel},
        gltf_assets::{
            GltfAsset,
            gltf_loader::loader::{GltfLoadError, GltfLoader},
            model_builder_new::GltfBuilder,
        },
    },
    util::types::{IndexType, ModelVertex},
};

impl GltfBuilder for Asset {}

impl GltfAsset for Asset {
    fn new_gltf(dir_name: &str) -> Result<Asset, GltfLoadError> {
        let (gltf, bin) = GltfLoader::load_gltf_from_resource(dir_name)?;
        Ok(Self::Gltf(gltf, bin))
    }
}

impl Asset {
    fn load<V: ModelVertex, I: IndexType>(
        &self,
        index_data_offset: usize,
        vertex_data_offset: usize,
    ) {
        match self {
            Self::Gltf(gltf, bin_source) => {
                Asset::load_gltf::<V, I>(&gltf, &bin_source, index_data_offset, vertex_data_offset)
                    .unwrap();
            }
            _ => panic!(),
        }
        todo!();
    }
}

struct RegisteredAsset {
    asset: Asset,
    residency_level: AssetResidencyLevel,
}

pub(super) struct AssetBuilderNew<V: ModelVertex, I: IndexType> {
    registered_assets: HashMap<AssetHandle, RegisteredAsset>,
    v: PhantomData<V>,
    i: PhantomData<I>,
}

impl<V: ModelVertex, I: IndexType> AssetBuilderNew<V, I> {
    pub fn new() -> Self {
        Self {
            registered_assets: HashMap::new(),
            v: PhantomData,
            i: PhantomData,
        }
    }
    pub fn register_asset(&mut self, asset: Asset, asset_handle: AssetHandle) {
        let registered_asset = RegisteredAsset {
            asset,
            residency_level: AssetResidencyLevel::Registered,
        };
        self.registered_assets
            .insert(asset_handle, registered_asset);
    }

    fn load_asset(&self, asset_handle: AssetHandle) {
        let asset = self.registered_assets.get(&asset_handle).unwrap();
    }
}
