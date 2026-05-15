#[cfg(test)]
mod rigid_tests {
    use std::{sync::Arc, vec};

    use crate::{
        animation::animation::{
            Animation, AnimationChannels, AnimationSampler, AnimationTransformType,
            AnimationTransforms, InterpolationType,
        },
        asset_manager::gltf_asset::{GltfAnimation, GltfNode, NodeTransforms, NodeType},
        world::instance_manager::AnimationInstance,
    };

    // buffer is column-major [[f32;4];4]: translation is column 3, so [3][0]=x, [3][1]=y, [3][2]=z

    #[allow(unused)]
    fn translation_trs(tx: f32, ty: f32, tz: f32) -> [NodeTransforms; 3] {
        [
            NodeTransforms::Translation(cgmath::Vector3::new(tx, ty, tz)),
            NodeTransforms::Rotation(cgmath::Quaternion::new(1.0, 0.0, 0.0, 0.0)),
            NodeTransforms::Scale(cgmath::Vector3::new(1.0, 1.0, 1.0)),
        ]
    }

    #[allow(unused)]
    fn identity_trs() -> [NodeTransforms; 3] {
        translation_trs(0.0, 0.0, 0.0)
    }

    #[allow(unused)]
    fn translation_sampler(times: Vec<f32>, keyframes: Vec<f32>) -> AnimationSampler {
        AnimationSampler::new(
            InterpolationType::Linear,
            times,
            AnimationTransforms(keyframes),
        )
    }
}
