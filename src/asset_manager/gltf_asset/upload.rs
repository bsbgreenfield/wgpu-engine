use std::{any::TypeId, sync::Arc};

use cgmath::SquareMatrix;

use crate::{
    animation::animation::{Animation, EntityAnimations},
    asset_manager::{
        AssetLoadError, ProvidesAnimationData, ProvidesMeshData,
        gltf_asset::{
            GltfAsset, GltfNode,
            util::{
                collect_mesh_instances, collect_mesh_instances_with_jts, get_root_node,
                skin_offset_of,
            },
        },
    },
    util::types::{LocalTransform, Mat4F32, PNUJWVertex, PNUVertex},
    world::{
        components::{AnimationAccessor, MeshAcessor, RigidAnimationMode},
        entity_manager::MeshRenderables,
    },
};

pub(super) struct MeshInstance {
    pub(super) mesh_id: usize,
    pub(super) local_transform: LocalTransform,
    pub(super) skin_idx: Option<usize>,
}

impl ProvidesMeshData for GltfAsset {
    fn render_mesh_data<'a>(
        &self,
        mesh_accesor: &'a MeshAcessor,
        _mode: &'a RigidAnimationMode,
    ) -> MeshRenderables {
        let mut jts: Vec<Vec<Mat4F32>> = self
            .skins
            .iter()
            .map(|skin| {
                skin.iter()
                    .map(|_joint| cgmath::Matrix4::<f32>::identity().into())
                    .collect()
            })
            .collect();
        let mesh_instances: Vec<MeshInstance> = match mesh_accesor {
            MeshAcessor::All => self
                .node_tree
                .iter()
                .flat_map(|node| {
                    collect_mesh_instances_with_jts(
                        node,
                        cgmath::Matrix4::<f32>::identity(),
                        &mut jts,
                    )
                })
                .collect(),
            MeshAcessor::GltfRootNode(root) => {
                match get_root_node(&self.node_tree, *root as usize) {
                    Some(root_node) => collect_mesh_instances_with_jts(
                        root_node,
                        cgmath::Matrix4::<f32>::identity(),
                        &mut jts,
                    ),
                    None => {
                        panic!()
                    }
                }
            }
        };
        let mut pnu_ranges = Vec::new();
        let mut pnu_mesh_map = Vec::new();
        let mut pnujw_mesh_map = Vec::new();
        let mut pnujw_ranges = Vec::new();
        let mut index_ranges = Vec::new();
        let mut local_transforms = Vec::new();
        let mut joint_map = Vec::new();
        let has_indices = self.meshes[0].primitives[0].indices.is_some();
        let mut relative_lt_offset = 0;
        // iterate over each mesh instance and its associate local transform
        // "relative_lt_offset" is the index of the local transform that corresponds to each
        // primitive draw. When iterating over the draw calls later on from i = 0..x_prims.len(),
        // the value located at x_mesh_map[i] will equal the correct offset within
        // the allocation's lt buffer range for x_prims[i]
        for mesh_instance in mesh_instances {
            let mesh = self
                .meshes
                .iter()
                .find(|m| m.id == mesh_instance.mesh_id as u32)
                .ok_or(AssetLoadError::InstanceUploadFailure(
                    "could not find mesh instance".to_string(),
                ))
                .expect("should have mesh id");
            for primitive in mesh.primitives.iter() {
                if primitive.vertex_type == TypeId::of::<PNUVertex>() {
                    pnu_mesh_map.push(relative_lt_offset);
                    pnu_ranges.push(primitive.vertices.clone());
                } else if primitive.vertex_type == TypeId::of::<PNUJWVertex>() {
                    pnujw_mesh_map.push(relative_lt_offset);
                    pnujw_ranges.push(primitive.vertices.clone());
                    joint_map.push(skin_offset_of(mesh_instance.skin_idx, &self.skins) as u32);
                } else {
                    panic!("vertex type not specified {:?}", primitive.vertex_type);
                }
                if has_indices {
                    let i = primitive.indices.clone().unwrap();
                    index_ranges.push(i)
                }
            }
            local_transforms.push(mesh_instance.local_transform);
            relative_lt_offset += 1;
        }
        println!("{pnu_mesh_map:?}");
        MeshRenderables {
            pnu_mesh_map,
            pnujw_mesh_map,
            pnu_vertex_ranges: (!pnu_ranges.is_empty()).then_some(pnu_ranges),
            pnujw_vertex_ranges: (!pnujw_ranges.is_empty()).then_some(pnujw_ranges),
            index_ranges: (!index_ranges.is_empty()).then_some(index_ranges),
            local_transforms,
            joint_transforms: jts.drain(..).flatten().collect(),
            joint_map,
        }
    }
}

impl ProvidesAnimationData for GltfAsset {
    fn entity_animation<'a>(
        &self,
        animation_accessor: &AnimationAccessor,
        mesh_accesor: &MeshAcessor,
    ) -> crate::animation::animation::EntityAnimations {
        let mesh_instances: Vec<(u32, LocalTransform)> = match mesh_accesor {
            MeshAcessor::All => self
                .node_tree
                .iter()
                .flat_map(|node| collect_mesh_instances(node, cgmath::Matrix4::<f32>::identity()))
                .collect(),
            MeshAcessor::GltfRootNode(root) => {
                match get_root_node(&self.node_tree, *root as usize) {
                    Some(root_node) => {
                        collect_mesh_instances(root_node, cgmath::Matrix4::<f32>::identity())
                    }
                    None => {
                        panic!()
                    }
                }
            }
        };
        let mut local_transforms = Vec::<LocalTransform>::with_capacity(mesh_instances.len());
        let mut mesh_indices = Vec::<usize>::with_capacity(mesh_instances.len());
        for (mesh_index, lt) in mesh_instances {
            local_transforms.push(lt);
            mesh_indices.push(mesh_index as usize);
        }
        let anim_refs: Vec<Arc<dyn Animation>> = match animation_accessor {
            AnimationAccessor::All => self
                .animations
                .iter()
                .map(|a| a.clone() as Arc<dyn Animation>)
                .collect(),
            AnimationAccessor::Index(idx) => {
                vec![self.animations[*idx].clone() as Arc<dyn Animation>]
            }
        };

        EntityAnimations {
            animation: anim_refs,
            local_transforms,
            mesh_slot_map: mesh_indices,
            joint_slot_map: todo!(),
            joint_transforms: todo!(),
        }
    }
}
