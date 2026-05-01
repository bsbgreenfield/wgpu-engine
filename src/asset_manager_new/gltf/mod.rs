use std::{any::TypeId, fmt::Display, path::PathBuf};

use crate::{
    app::{GPUAssetUploadJob, renderer::GPUAllocationHandle},
    asset_manager_new::{
        Asset, AssetHandle, AssetLoadError, LoadedAsset, ModelBuilderError, gltf::mesh::Mesh,
    },
    util::types::{LocalTransform, MAT4_IDENTITY, Mat4F32, PNUJWVertex, PNUVertex, VIndex},
    world::{
        InstanceUploadQuery,
        components::{MeshAcessor, RigidAnimationMode},
        entity_manager::{RenderData, Renderables},
        instance_manager::InstanceHandle,
        world::{InstanceUploadData, LocalTransformData},
    },
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
    indices: Option<Vec<VIndex>>,
}

fn get_root_node(nodes: &[GltfNode], node_id: usize) -> Option<&GltfNode> {
    for node in nodes {
        if node.node_id == node_id {
            return Some(node);
        }
        if let Some(found) = get_root_node(&node.children, node_id) {
            return Some(found);
        }
    }
    None
}

fn collect_mesh_instances(
    node: &GltfNode,
    parent_transform: Mat4F32,
) -> Vec<(u32, LocalTransform)> {
    let mut result = Vec::<(u32, LocalTransform)>::new();
    use cgmath::Matrix4;
    let accumulated: Mat4F32 =
        (Matrix4::from(parent_transform) * Matrix4::from(node.transform)).into();
    if let Some(mesh_id) = node.mesh_id {
        result.push((mesh_id as u32, accumulated.into()));
    }
    for child in &node.children {
        result.extend(collect_mesh_instances(child, accumulated));
    }
    result
}
fn collect_local_transforms(node: &GltfNode, parent_transform: Mat4F32) -> Vec<LocalTransform> {
    use cgmath::Matrix4;
    let accumulated: Mat4F32 =
        (Matrix4::from(parent_transform) * Matrix4::from(node.transform)).into();

    let mut result = Vec::new();
    if node.mesh_id.is_some() {
        result.push(accumulated.into());
    }
    for child in &node.children {
        result.extend(collect_local_transforms(child, accumulated));
    }
    result
}

impl LoadedAsset for LoadedGltfAsset {
    fn upload_job<'a>(
        &'a self,
        asset_handle: AssetHandle,
    ) -> Result<GPUAssetUploadJob<'a>, AssetLoadError> {
        GPUAssetUploadJob::new(
            asset_handle,
            Some(&self.pnu_vertices[..]),
            Some(&self.pnujw_vertices[..]),
            self.indices.as_deref(),
        )
    }

    // fn get_instance_upload_data<'a>(
    //     &'a self,
    //     instance_handle: InstanceHandle,
    //     mesh_accessor: &MeshAcessor,
    // ) -> crate::world::world::InstanceUploadData {
    //     let local_transforms = match mesh_accessor {
    //         MeshAcessor::All => self
    //             .node_tree
    //             .iter()
    //             .flat_map(|node| collect_local_transforms(node, MAT4_IDENTITY))
    //             .collect(),
    //         MeshAcessor::GltfRootNode(root) => {
    //             match get_root_node(&self.node_tree, *root as usize) {
    //                 Some(root_node) => collect_local_transforms(root_node, MAT4_IDENTITY),
    //                 None => {
    //                     panic!()
    //                 }
    //             }
    //         }
    //     };
    //     InstanceUploadData {
    //         instance_handle,
    //         local_transforms: Some(local_transforms),
    //     }
    // }
    fn get_renderables(
        &self,
        alloc_handle: GPUAllocationHandle,
        renderables: &mut Renderables,
        query: &InstanceUploadQuery,
    ) -> Result<(), AssetLoadError> {
        if query.needs_meshes && query.needs_local_transforms {
            let mesh_instances: Vec<(u32, LocalTransform)> = match query.mesh_accesor.unwrap() {
                MeshAcessor::All => self
                    .node_tree
                    .iter()
                    .flat_map(|node| collect_mesh_instances(node, MAT4_IDENTITY))
                    .collect(),
                MeshAcessor::GltfRootNode(root) => {
                    match get_root_node(&self.node_tree, *root as usize) {
                        Some(root_node) => collect_mesh_instances(root_node, MAT4_IDENTITY),
                        None => {
                            return Err(AssetLoadError::InstanceUploadFailure(String::from(
                                "The root node defined for this entity is not valid for the asset",
                            )));
                        }
                    }
                }
            };
            let mut pnu_ranges = Vec::new();
            let mut pnujw_ranges = Vec::new();
            let mut index_ranges = Vec::new();
            let mut local_transforms = Vec::new();
            let has_indices = self.meshes[0].primitives[0].indices.is_some();
            for (mesh_id, local_transform) in mesh_instances {
                let mesh = self.meshes.iter().find(|m| m.id == mesh_id).ok_or(
                    AssetLoadError::InstanceUploadFailure(
                        "could not find mesh instance".to_string(),
                    ),
                )?;
                for primitive in mesh.primitives.iter() {
                    if primitive.vertex_type == TypeId::of::<PNUVertex>() {
                        pnu_ranges.push(primitive.vertices.clone());
                    } else if primitive.vertex_type == TypeId::of::<PNUJWVertex>() {
                        pnujw_ranges.push(primitive.vertices.clone());
                    } else {
                        panic!("vertex type not specified {:?}", primitive.vertex_type);
                    }
                    if has_indices {
                        let i = primitive.indices.clone().unwrap();
                        index_ranges.push(i)
                    }
                    local_transforms.push(local_transform);
                }
            }

            let mesh_render_data = RenderData::MeshRenderable {
                gpu_alloc_handle: alloc_handle,
                pnu_vertex_ranges: (!pnu_ranges.is_empty()).then_some(pnu_ranges),
                pnujw_vertex_ranges: (!pnujw_ranges.is_empty()).then_some(pnujw_ranges),
                index_ranges: (!index_ranges.is_empty()).then_some(index_ranges),
            };
            // INSERT COMMON MESH DATA
            if let Some(common) = renderables.common.as_mut() {
                common.push(mesh_render_data);
            } else {
                let _ = renderables.common.insert(vec![mesh_render_data]);
            }
            // INSERT LOCAL TRANSFORMS
            if let Some(instance_data) = renderables.instance_data.as_mut() {
                instance_data.local_transforms = LocalTransformData::FromVec(local_transforms);
            } else {
                renderables.instance_data = Some(InstanceUploadData {
                    instance_handle: renderables.instance_handle.clone(),
                    local_transforms: LocalTransformData::FromVec(local_transforms),
                })
            }
        } else if query.needs_local_transforms {
            let local_transforms = match query.mesh_accesor.unwrap() {
                MeshAcessor::All => self
                    .node_tree
                    .iter()
                    .flat_map(|node| collect_local_transforms(node, MAT4_IDENTITY))
                    .collect(),
                MeshAcessor::GltfRootNode(root) => {
                    match get_root_node(&self.node_tree, *root as usize) {
                        Some(root_node) => collect_local_transforms(root_node, MAT4_IDENTITY),
                        None => {
                            return Err(AssetLoadError::InstanceUploadFailure(String::from(
                                "The root node defined for this entity is not valid for the asset",
                            )));
                        }
                    }
                }
            };

            // INSERT LOCAL TRANSFORMS
            if let Some(instance_data) = renderables.instance_data.as_mut() {
                instance_data.local_transforms = LocalTransformData::FromVec(local_transforms);
            } else {
                renderables.instance_data = Some(InstanceUploadData {
                    instance_handle: renderables.instance_handle.clone(),
                    local_transforms: LocalTransformData::FromVec(local_transforms),
                });
            }
        } else {
            // the instance spawned, but it does NOT require local transforms, interesting!
            assert!(
                matches!(query.rigid_animation_mode, Some(RigidAnimationMode::Shared)),
                "the instance DOESNT need local transforms but ISNT shared?"
            );
            if let Some(instance_data) = renderables.instance_data.as_mut() {
                instance_data.local_transforms = LocalTransformData::NeedsDonor;
            } else {
                renderables.instance_data = Some(InstanceUploadData {
                    instance_handle: renderables.instance_handle.clone(),
                    local_transforms: LocalTransformData::NeedsDonor,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum GltfValidationError {
    NoView,
    UnsupportedScheme,
}
#[derive(Debug)]
pub enum GltfLoadError {
    IOErr(std::io::ErrorKind),
    InvalidFileError,
    MultipleFileTypes,
    GltfNeedsBinFile,
    GltfPackageError(gltf::Error),
    BadFile(String),
    ModelBuilderError(Box<ModelBuilderError>),
    Unimplemented,
}

impl Display for GltfLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOErr(err) => err.fmt(f),
            Self::InvalidFileError => f.write_str("Gltf load failed due to an invald file type"),
            Self::MultipleFileTypes => f.write_str("Gltf load failed due to there being multiple file types to choose from in the provided asset source file"),
            Self::GltfNeedsBinFile => f.write_str("Gltf load failed due to a missing bin file for the associated gltf file"),
            Self::GltfPackageError(err) => err.fmt(f),
            Self::BadFile(str) => f.write_str(str),
            Self::ModelBuilderError(_) => f.write_str("Gltf load failed internally"),
            Self::Unimplemented => f.write_str("This type of gltf loading has not been implemented"),
        }
    }
}
impl std::error::Error for GltfLoadError {}

impl From<ModelBuilderError> for GltfLoadError {
    fn from(value: ModelBuilderError) -> Self {
        Self::ModelBuilderError(Box::new(value))
    }
}

impl From<gltf::Error> for GltfLoadError {
    fn from(value: gltf::Error) -> Self {
        Self::GltfPackageError(value)
    }
}
