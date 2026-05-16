use std::sync::Arc;

use crate::{
    asset_manager::gltf_asset::{GltfNode, NodeTransforms, NodeType, upload::MeshInstance},
    util::types::{LocalTransform, Mat4F32},
};

pub(super) fn get_root_node(nodes: &[Arc<GltfNode>], node_id: usize) -> Option<&GltfNode> {
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

pub(super) fn collect_mesh_ids(node: &GltfNode, mesh_list: &mut Vec<usize>) {
    if let NodeType::Mesh(mesh_id) = node.node_type {
        mesh_list.push(mesh_id);
    }
    for child in node.children.iter() {
        collect_mesh_ids(child, mesh_list);
    }
}

pub(super) fn skin_offset_of(skin_idx: Option<usize>, skins: &Vec<Vec<usize>>) -> usize {
    if let Some(idx) = skin_idx {
        if idx == 0 {
            return 0;
        } else {
            let mut offset = 0;
            for i in 0..idx {
                offset += skins[i].len();
            }
            return offset;
        }
    }
    return 0;
}

pub(super) fn collect_mesh_instances_with_jts(
    node: &GltfNode,
    parent_transform: cgmath::Matrix4<f32>,
    jts: &mut Vec<Vec<Mat4F32>>,
) -> Vec<MeshInstance> {
    let mut result = Vec::<MeshInstance>::new();
    use cgmath::Matrix4;
    let accumulated: cgmath::Matrix4<f32> =
        Matrix4::from(parent_transform) * NodeTransforms::to_matrix(&node.transform_components);

    if let NodeType::Mesh(mesh_id) = node.node_type {
        result.push(MeshInstance {
            mesh_id,
            local_transform: accumulated.into(),
            skin_idx: node.skin_idx,
        });
    } else if let NodeType::Joint((skin_id, joint_id)) = node.node_type {
        jts[skin_id as usize][joint_id as usize] = accumulated.into();
    }
    for child in &node.children {
        result.extend(collect_mesh_instances_with_jts(child, accumulated, jts));
    }
    result
}
pub(super) fn collect_mesh_instances(
    node: &GltfNode,
    parent_transform: cgmath::Matrix4<f32>,
) -> Vec<(u32, LocalTransform)> {
    let mut result = Vec::<(u32, LocalTransform)>::new();
    use cgmath::Matrix4;
    let accumulated: cgmath::Matrix4<f32> =
        Matrix4::from(parent_transform) * NodeTransforms::to_matrix(&node.transform_components);

    if let NodeType::Mesh(mesh_id) = node.node_type {
        result.push((mesh_id as u32, accumulated.into()));
    }
    for child in &node.children {
        result.extend(collect_mesh_instances(child, accumulated));
    }
    result
}
