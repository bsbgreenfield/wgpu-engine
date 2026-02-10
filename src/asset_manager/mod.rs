pub mod asset_manager;
pub(super) mod gltf_assets;
mod range_splicer;
pub enum Asset {
    Gltf(gltf::Gltf, gltf_assets::gltf_loader::loader::BinarySource),
    Other,
}
