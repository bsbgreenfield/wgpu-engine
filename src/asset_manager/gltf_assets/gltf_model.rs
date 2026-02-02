use std::marker::PhantomData;

use crate::{
    asset_manager::{
        asset_manager::{Asset, AssetLoadError},
        gltf_assets::model_builder_new::GltfModelBuilderNew,
    },
    util::types::{IndexType, ModelVertex},
    world::components::MeshCollectionComponent,
};

pub struct GltfAsset<V: ModelVertex, I: IndexType> {
    vertex_type: PhantomData<V>,
    index_type: PhantomData<I>,
}

impl<V: ModelVertex, I: IndexType> Asset for GltfAsset<V, I> {
    type Builder = GltfModelBuilderNew<V, I>;
    type Components = (MeshCollectionComponent,);
    fn builder(dir_name: &str) -> Result<Self::Builder, AssetLoadError> {
        GltfModelBuilderNew::<V, I>::new(dir_name)
    }
}
