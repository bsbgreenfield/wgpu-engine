use std::{any::TypeId, path::PathBuf};

use crate::{
    app::GPUUploadJob,
    asset_manager_new::{Asset, AssetLoadError, AssetResidency, LoadedAsset, gltf::mesh::Mesh},
    util::types::{Mat4F32, PNUJWVertex, PNUVertex},
};
mod build;
mod loader;
mod mesh;

#[allow(unused)]
#[derive(Clone)]
pub(in crate::asset_manager_new) enum BinarySource {
    BinFile(PathBuf),
    GLB(PathBuf),
    GLTFBuffers(PathBuf),
    Undefined,
}
pub struct GltfAsset {
    gltf: gltf::Gltf,
    bin: BinarySource,
    res_level: AssetResidency,
}

impl Asset for GltfAsset {
    fn new(dir_name: &str) -> Result<Self, super::AssetLoadError>
    where
        Self: Sized,
    {
        let res = super::gltf::loader::load_gltf_from_resource(dir_name)?;
        Ok(Self {
            gltf: res.0,
            bin: res.1,
            res_level: AssetResidency::Registered,
        })
    }
}
struct GltfNode {
    mesh_id: Option<usize>,
    node_id: usize,
    children: Vec<GltfNode>,
    transform: Mat4F32,
}

pub(super) struct LoadedGltfAsset {
    node_tree: Vec<GltfNode>,
    meshes: Vec<Mesh>,
    pnujw_vertices: Vec<PNUJWVertex>,
    pnu_vertices: Vec<PNUVertex>,
}

impl LoadedAsset for LoadedGltfAsset {
    fn upload_job<'a>(&'a self) -> Result<GPUUploadJob<'a>, AssetLoadError> {
        todo!()
    }
    fn get_renderables(&self) -> Option<crate::world::entity_manager::Renderables> {
        let mut pnu_ranges = Vec::new();
        let mut pnujw_ranges = Vec::new();

        for mesh in self.meshes.iter() {
            for primitive in mesh.primitives.iter() {
                if primitive.vertex_type == TypeId::of::<PNUVertex>() {
                    pnu_ranges.push(primitive.vertices.clone());
                } else if primitive.vertex_type == TypeId::of::<PNUVertex>() {
                    pnujw_ranges.push(primitive.vertices.clone());
                } else {
                    panic!("vertex type not specified {:?}", primitive.vertex_type);
                }

                todo!("INDEX RANGES")
            }
        }

        todo!()
    }
}
