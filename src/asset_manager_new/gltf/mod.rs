use std::{
    any::TypeId,
    fmt::{Debug, Display},
    hash::Hash,
    ops::Deref,
    path::PathBuf,
    sync::Arc,
};

use cgmath::{Matrix4, Quaternion};

use crate::{
    animation::animation::{
        Animation, AnimationChannel, AnimationChannels, AnimationSampler, AnimationTransformType,
        SampleResult,
    },
    app::{GPUAssetUploadJob, renderer::GPUAllocationHandle},
    asset_manager_new::{
        Asset, AssetHandle, AssetLoadError, LoadedAsset, ModelBuilderError, gltf::mesh::Mesh,
    },
    util::types::{LocalTransform, MAT4_IDENTITY, Mat4F32, PNUJWVertex, PNUVertex, VIndex},
    world::{
        InstanceUploadQuery,
        components::{AnimationAccessor, MeshAcessor, RigidAnimationMode},
        entity_manager::{RenderData, Renderables},
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
#[derive(Debug)]
struct GltfNode {
    mesh_id: Option<usize>,
    node_id: usize,
    children: Vec<GltfNode>,
    transform: Mat4F32,
}

impl PartialEq for GltfNode {
    fn eq(&self, other: &Self) -> bool {
        self.mesh_id == other.mesh_id
    }
}

impl Eq for GltfNode {}

impl Hash for GltfNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node_id.hash(state);
    }
}

pub(super) struct LoadedGltfAsset {
    node_tree: Vec<Arc<GltfNode>>,
    meshes: Vec<Mesh>,
    pnujw_vertices: Vec<PNUJWVertex>,
    pnu_vertices: Vec<PNUVertex>,
    indices: Option<Vec<VIndex>>,
    animations: Vec<Arc<GltfAnimation>>,
}

fn get_root_node(nodes: &[Arc<GltfNode>], node_id: usize) -> Option<&GltfNode> {
    for node in nodes {
        if node.node_id == node_id {
            return Some(node);
        }
        if let Some(found) = find_in_children(&node.children, node_id) {
            return Some(found);
        }
    }
    None
}

fn find_in_children(chidlren: &[GltfNode], node_id: usize) -> Option<&GltfNode> {
    for node in chidlren {
        if node.node_id == node_id {
            return Some(node);
        }
        if let Some(found) = find_in_children(&node.children, node_id) {
            return Some(found);
        }
    }
    None
}

//pub(super) fn get_root_node_from_child_id(roots: &[GltfNode], child_id: usize) -> Option<usize> {
//    for (i, root) in roots.iter().enumerate() {
//        if root.node_id == child_id {
//            return Some(i);
//        }
//        if get_root_node(&root.children, child_id).is_some() {
//            return Some(i);
//        }
//    }
//    None
//}
pub(super) fn get_root_node_from_child_id(
    roots: &[Arc<GltfNode>],
    child_id: usize,
) -> Option<Arc<GltfNode>> {
    for root in roots.iter() {
        if root.node_id == child_id {
            return Some(root.clone());
        }
        if get_root_node(&root.children, child_id).is_some() {
            return Some(root.clone());
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
            // the instance spawned, but it does NOT require local transforms, must be shared
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

        // GET ANIMATIONS
        if query.needs_animations {
            match query.animation_accessor.unwrap() {
                AnimationAccessor::Index(idx) => {
                    renderables
                        .common
                        .as_mut()
                        .unwrap()
                        .push(RenderData::AnimationData {
                            animation: vec![self.animations[*idx].clone()],
                        });
                }
                AnimationAccessor::All => {
                    let mut animation_refs =
                        Vec::<Arc<dyn Animation>>::with_capacity(self.animations.len());
                    for anim in self.animations.iter() {
                        animation_refs.push(anim.clone());
                    }
                    renderables
                        .common
                        .as_mut()
                        .unwrap()
                        .push(RenderData::AnimationData {
                            animation: animation_refs,
                        });
                }
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

pub enum GltfAttributeType {
    Position,
    Normal,
    TexCoords,
    Index,
    Joints,
    Weights,
    IBMS,
    Times,
    RotationT,
    TranslationT,
    ScaleT,
}

impl GltfAttributeType {
    pub fn from_animation_channel(channel: &gltf::animation::Channel) -> Self {
        match channel.target().property() {
            gltf::animation::Property::Translation => Self::TranslationT,
            gltf::animation::Property::Rotation => Self::RotationT,
            gltf::animation::Property::Scale => Self::ScaleT,
            _ => panic!(),
        }
    }
}

impl Display for GltfLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOErr(err) => Display::fmt(err, f),
            Self::InvalidFileError => f.write_str("Gltf load failed due to an invald file type"),
            Self::MultipleFileTypes => f.write_str("Gltf load failed due to there being multiple file types to choose from in the provided asset source file"),
            Self::GltfNeedsBinFile => f.write_str("Gltf load failed due to a missing bin file for the associated gltf file"),
            Self::GltfPackageError(err) => Display::fmt(err, f),
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

pub struct GltfAnimation {
    pub root_nodes: Vec<Arc<GltfNode>>,
    pub samplers: Vec<AnimationSampler>,
    pub channels: AnimationChannels,
}

impl Debug for GltfAnimation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GltfAnimation")
            .field("root_nodes", &self.root_nodes)
            .field("samplers", &self.samplers.len())
            .field("channels", &self.channels.len())
            .finish()
    }
}

impl Animation for GltfAnimation {
    fn get_animation_frame(
        &self,
        time_delta: f32,
        instance: &mut crate::world::instance_manager::AnimationInstance,
    ) {
        for node in self.root_nodes.iter() {
            let mut rotation: Option<Quaternion<f32>> = None;
            let mut translation: Option<Matrix4<f32>> = None;
            let mut scale: Option<Matrix4<f32>> = None;
            if let Some(samplers) = self.channels.get(&node.node_id) {
                let sample_result = instance
                    .samples
                    .get_mut(&node.node_id)
                    .unwrap()
                    .sample(time_delta);

                match sample_result {
                    SampleResult::Done => continue,
                    SampleResult::Active(i) => {
                        for (sampler_idx, anim_type) in samplers {
                            let sampler = &self.samplers[*sampler_idx];
                            let ratio =
                                (time_delta - instance.start_time.as_secs_f32() - sampler.times[i])
                                    / (sampler.times[i + 1] - sampler.times[i]);
                            match anim_type {
                                AnimationTransformType::Rotation => {
                                    rotation =
                                        Some(interpolate_as_quats(i, ratio, &sampler.transforms.0));
                                }
                                AnimationTransformType::Translation => {
                                    translation = Some(cgmath::Matrix4::from_translation(
                                        interpolate_as_vec3(i, ratio, &sampler.transforms.0),
                                    ));
                                }
                                AnimationTransformType::Scale => {
                                    let s = interpolate_as_vec3(i, ratio, &sampler.transforms.0);
                                    scale =
                                        Some(cgmath::Matrix4::from_nonuniform_scale(s.x, s.y, s.z));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn interpolate_as_quats(cursor: usize, ratio: f32, floats: &Vec<f32>) -> cgmath::Quaternion<f32> {
    let quats: &[[f32; 4]] = bytemuck::cast_slice(floats);
    let q0 = quats[cursor];
    let q1 = quats[cursor + 1];

    let dot = q0[0] * q1[0] + q0[1] * q1[1] + q0[2] * q1[2] + q0[3] * q1[3];
    let q1 = if dot < 0.0 {
        [-q1[0], -q1[1], -q1[2], -q1[3]]
    } else {
        q1
    };
    let x = q0[0] + ratio * (q1[0] - q0[0]);
    let y = q0[1] + ratio * (q1[1] - q0[1]);
    let z = q0[2] + ratio * (q1[2] - q0[2]);
    let w = q0[3] + ratio * (q1[3] - q0[3]);
    let inv_len = 1.0 / (x * x + y * y + z * z + w * w).sqrt();
    cgmath::Quaternion::new(w * inv_len, x * inv_len, y * inv_len, z * inv_len)
}
fn interpolate_as_vec3(cursor: usize, ratio: f32, floats: &[f32]) -> cgmath::Vector3<f32> {
    let vecs: &[[f32; 3]] = bytemuck::cast_slice(floats);
    let v0 = vecs[cursor];
    let v1 = vecs[cursor + 1];

    cgmath::Vector3::new(
        v0[0] + ratio * (v1[0] - v0[0]),
        v0[1] + ratio * (v1[1] - v0[1]),
        v0[2] + ratio * (v1[2] - v0[2]),
    )
}
