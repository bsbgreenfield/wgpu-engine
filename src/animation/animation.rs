use std::collections::HashMap;

enum AnimationTransform {
    Rotation(Vec<cgmath::Quaternion<f32>>),
    Translation(Vec<cgmath::Vector3<f32>>),
    Scale(Vec<cgmath::Vector3<f32>>),
}

struct NodeTransform {
    rot: cgmath::Quaternion<f32>,
    trans: cgmath::Vector3<f32>,
    scale: cgmath::Vector3<f32>,
}
impl NodeTransform {
    fn new(
        rot: cgmath::Quaternion<f32>,
        trans: cgmath::Vector3<f32>,
        scale: cgmath::Vector3<f32>,
    ) -> Self {
        Self { rot, trans, scale }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InterpolationType {
    Linear,
}
impl From<gltf::animation::Interpolation> for InterpolationType {
    fn from(value: gltf::animation::Interpolation) -> Self {
        match value {
            gltf::animation::Interpolation::Linear => InterpolationType::Linear,
            _ => todo!(),
        }
    }
}
struct AnimationSampler {
    id: usize,
    interp: InterpolationType,
    times: Vec<f32>,
    transform: AnimationTransform,
}

type AnimationSamplerMap = HashMap<usize, Vec<AnimationSampler>>;

#[derive(Debug, PartialEq, Eq, PartialOrd)]
pub enum NodeType {
    Node,
    Mesh,
    Joint(usize),
}
struct AnimationNode {
    samplers: AnimationSamplerMap,
    children: Vec<AnimationNode>,
    transform: NodeTransform,
    node_type: NodeType,
    node_id: usize,
}
