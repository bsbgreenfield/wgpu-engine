use std::{collections::HashMap, sync::Arc};

use crate::asset_manager_new::gltf::GltfAttributeType;

//#[allow(unused)]
//pub enum AnimationTransform {
//    Rotation(Vec<cgmath::Quaternion<f32>>),
//    Translation(Vec<cgmath::Vector3<f32>>),
//    Scale(Vec<cgmath::Vector3<f32>>),
//}

impl AnimationTransforms {
    pub fn from_bytes(attribute_type: GltfAttributeType, bytes: Vec<u8>) -> Self {
        let f32_vec: Vec<f32> = bytemuck::cast_vec(bytes);
        match attribute_type {
            GltfAttributeType::RotationT => {
                let mut quat_vec: Vec<[f32; 4]> = Vec::new();
                for i in 0..f32_vec.len() / 4 {
                    let quat_slice: [f32; 4] = f32_vec[i * 4..i * 4 + 4].try_into().unwrap();
                    quat_vec.push(quat_slice);
                }
                Self(quat_vec)
            }
            GltfAttributeType::TranslationT => {
                let mut trans_vec: Vec<[f32; 4]> = Vec::new();
                for i in 0..f32_vec.len() / 3 {
                    let trans_slice: [f32; 3] = f32_vec[i * 3..i * 3 + 3].try_into().unwrap();
                    let padded: [f32; 4] = [trans_slice[0], trans_slice[1], trans_slice[2], 0.0];
                    trans_vec.push(padded);
                }

                Self(trans_vec)
            }
            GltfAttributeType::ScaleT => {
                let mut scale_vec: Vec<[f32; 4]> = Vec::new();
                for i in 0..f32_vec.len() / 3 {
                    let scale_slice: &[f32; 3] = &f32_vec[i * 3..i * 3 + 3].try_into().unwrap();
                    let padded: [f32; 4] = [scale_slice[0], scale_slice[1], scale_slice[2], 0.0];
                    scale_vec.push(padded);
                }

                Self(scale_vec)
            }
            _ => panic!("unable to create a transform from this attribute type"),
        }
    }
}

#[repr(C)]
pub struct AnimationTransforms(Vec<[f32; 4]>);
pub enum AnimationTransformType {
    Rotation,
    Translation,
    Scale,
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
    interp: InterpolationType,
    times: Vec<f32>,
    transforms: AnimationTransforms,
}

impl AnimationSampler {
    pub fn new(
        interp: InterpolationType,
        times: Vec<f32>,
        transforms: AnimationTransforms,
    ) -> Self {
        Self {
            interp,
            times,
            transforms,
        }
    }
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

pub struct AnimationChannel {
    target: u32,
    transform_type: AnimationTransformType,
}

impl AnimationChannel {
    pub fn from_gltf_channel(channel: &gltf::animation::Channel) -> Self {
        let transform_type = match channel.target().property() {
            gltf::animation::Property::Translation => AnimationTransformType::Translation,
            gltf::animation::Property::Scale => AnimationTransformType::Scale,
            gltf::animation::Property::Rotation => AnimationTransformType::Rotation,
            _ => todo!(),
        };
        Self {
            target: channel.target().node().index() as u32,
            transform_type,
        }
    }
}

pub struct AnimationSample {
    end_time: f32,
    transform_index: i32,
}

pub struct AnimationInstance<A: Animation> {
    animation: A,
    samples: Vec<AnimationSample>,
}
