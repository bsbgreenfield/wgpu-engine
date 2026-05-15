use std::sync::Arc;

use crate::{
    asset_manager::gltf_asset::{GltfNode, NodeTransforms, NodeType},
    util::types::LocalTransform,
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
