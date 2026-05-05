use std::{collections::HashMap, fmt::Debug, marker::PhantomData, mem::MaybeUninit, sync::Arc};

use crate::{
    asset_manager_new::gltf::GltfAttributeType, world::instance_manager::AnimationInstance,
};

//#[allow(unused)]
//pub enum AnimationTransform {
//    Rotation(Vec<cgmath::Quaternion<f32>>),
//    Translation(Vec<cgmath::Vector3<f32>>),
//    Scale(Vec<cgmath::Vector3<f32>>),
//}

#[repr(C)]
pub struct AnimationTransforms(pub Vec<f32>);

pub enum AnimationTransformType {
    Rotation,
    Translation,
    Scale,
}

impl AnimationTransformType {
    pub fn from_gltf_prop(prop: &gltf::animation::Property) -> Self {
        match prop {
            gltf::animation::Property::Translation => Self::Translation,
            gltf::animation::Property::Scale => Self::Scale,
            gltf::animation::Property::Rotation => Self::Rotation,
            _ => todo!(),
        }
    }
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
    pub interp: InterpolationType,
    pub times: Vec<f32>,
    pub transforms: AnimationTransforms,
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

pub type AnimationChannels = HashMap<usize, Vec<(usize, AnimationTransformType)>>;

pub struct AnimationSample {
    pub complete: bool,
    next_time: f32,
    end_time: f32,
    pub cursor: usize,
}

impl AnimationSample {
    pub fn init(times: &[f32]) -> Self {
        Self {
            complete: false,
            next_time: times[1],
            end_time: times[times.len()],
            cursor: 0,
        }
    }
}

pub enum SampleResult {
    Done,
    Active(usize),
    End,
}
impl AnimationSample {
    pub fn sample(&mut self, time_delta: f32) -> SampleResult {
        if self.complete {
            return SampleResult::Done;
        } else if time_delta >= self.end_time {
            self.complete = true;
            return SampleResult::Done;
        } else if time_delta >= self.next_time {
            self.cursor += 1;
        }
        SampleResult::Active(self.cursor)
    }
}

pub trait Animation
where
    Self: Debug,
{
    fn get_animation_frame(
        &self,
        time_delta: f32,
        animation_instance: &mut AnimationInstance,
        base_translation: &cgmath::Matrix4<f32>,
    );

    fn count(&self) -> usize;

    fn get_buffer_slot(&self, id: usize) -> usize;

    fn init_samples(&self) -> HashMap<usize, AnimationSample>;
}
