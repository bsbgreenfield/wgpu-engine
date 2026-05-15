use std::{any::TypeId, sync::Arc};

use cgmath::SquareMatrix;

use crate::{
    animation::animation::{Animation, EntityAnimations},
    asset_manager::{
        AssetLoadError, ProvidesAnimationData, ProvidesMeshData,
        gltf_asset::{
            GltfAsset,
            util::{collect_mesh_instances, get_root_node},
        },
    },
    util::types::{LocalTransform, PNUJWVertex, PNUVertex},
    world::{
        components::{AnimationAccessor, MeshAcessor, RigidAnimationMode},
        entity_manager::MeshRenderables,
    },
};

impl ProvidesMeshData for GltfAsset {
    fn render_mesh_data<'a>(
        &self,
        mesh_accesor: &'a MeshAcessor,
        _mode: &'a RigidAnimationMode,
    ) -> MeshRenderables {
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
        let mut pnu_ranges = Vec::new();
        let mut pnu_mesh_map = Vec::new();
        let mut pnujw_mesh_map = Vec::new();
        let mut pnujw_ranges = Vec::new();
        let mut index_ranges = Vec::new();
        let mut local_transforms = Vec::new();
        let has_indices = self.meshes[0].primitives[0].indices.is_some();
        let mut mesh_count = 0;
        for (mesh_id, local_transform) in mesh_instances {
            let mesh = self
                .meshes
                .iter()
                .find(|m| m.id == mesh_id)
                .ok_or(AssetLoadError::InstanceUploadFailure(
                    "could not find mesh instance".to_string(),
                ))
                .expect("should have mesh id");
            for primitive in mesh.primitives.iter() {
                if primitive.vertex_type == TypeId::of::<PNUVertex>() {
                    pnu_mesh_map.push(mesh_count);
                    pnu_ranges.push(primitive.vertices.clone());
                } else if primitive.vertex_type == TypeId::of::<PNUJWVertex>() {
                    pnujw_mesh_map.push(mesh_count);
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
            mesh_count += 1;
        }
        MeshRenderables {
            pnu_mesh_map,
            pnujw_mesh_map,
            pnu_vertex_ranges: (!pnu_ranges.is_empty()).then_some(pnu_ranges),
            pnujw_vertex_ranges: (!pnujw_ranges.is_empty()).then_some(pnujw_ranges),
            index_ranges: (!index_ranges.is_empty()).then_some(index_ranges),
            local_transforms,
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
        for (i, lt) in mesh_instances {
            local_transforms.push(lt);
            mesh_indices.push(i as usize);
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
            buffer_slot_map: mesh_indices,
        }
    }
}
