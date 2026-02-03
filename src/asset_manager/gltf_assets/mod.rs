pub(super) mod gltf_loader;
pub mod mesh;
pub mod model_builder_new;
mod primitive;
pub trait GltfAsset {
    fn new_gltf(dir_name: &str) -> Result<super::Asset, gltf_loader::loader::GltfLoadError>;
}
