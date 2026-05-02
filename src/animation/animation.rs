use std::{collections::HashMap, sync::Arc};

#[allow(unused)]
enum AnimationTransform {
    Rotation(Vec<cgmath::Quaternion<f32>>),
    Translation(Vec<cgmath::Vector3<f32>>),
    Scale(Vec<cgmath::Vector3<f32>>),
}

#[allow(unused)]
struct NodeTransform {
    rot: cgmath::Quaternion<f32>,
    trans: cgmath::Vector3<f32>,
    scale: cgmath::Vector3<f32>,
}
#[allow(unused)]
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
#[allow(unused)]
pub struct AnimationSampler {
    id: usize,
    interp: InterpolationType,
    times: Vec<f32>,
    transform: AnimationTransform,
}

#[allow(unused)]
type AnimationSamplerMap = HashMap<usize, Vec<AnimationSampler>>;

#[derive(Debug, PartialEq, Eq, PartialOrd)]
pub enum NodeType {
    Node,
    Mesh,
    Joint(usize),
}
#[allow(unused)]
struct AnimationNode {
    samplers: AnimationSamplerMap,
    children: Vec<AnimationNode>,
    transform: NodeTransform,
    node_type: NodeType,
    node_id: usize,
}

pub trait Animation {
    // TODO: get_animation_frame
}

pub struct AnimationSample {
    end_time: f32,
    transform_index: i32,
}

pub struct AnimationInstance<A: Animation> {
    animation: A,
    samples: Vec<AnimationSample>,
}
