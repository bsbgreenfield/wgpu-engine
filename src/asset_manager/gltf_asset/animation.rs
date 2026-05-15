use cgmath::{Quaternion, SquareMatrix, Vector3};

#[cfg(test)]
use crate::animation::animation::{AnimationChannels, AnimationSampler};
use crate::{
    animation::animation::{Animation, AnimationSample, AnimationTransformType, SampleResult},
    asset_manager::gltf_asset::{GltfAnimation, GltfNode, NodeTransforms, NodeType},
    world::instance_manager::AnimationInstance,
};

impl std::fmt::Debug for GltfAnimation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GltfAnimation")
            .field("samplers", &self.samplers.len())
            .field("channels", &self.channels.len())
            .finish()
    }
}

fn get_animation_data_for_node(
    node: &GltfNode,
    base_transform: &cgmath::Matrix4<f32>,
    time_delta: f32,
    animation: &GltfAnimation,
    animation_instance: &mut AnimationInstance,
    buffer_slot_map: &Vec<usize>,
) {
    let mut rotation: Option<Quaternion<f32>> = None;
    let mut translation: Option<Vector3<f32>> = None;
    let mut scale: Option<Vector3<f32>> = None;

    if let Some(node_channels) = animation.channels.get(&node.node_id) {
        for (sampler_count, (sampler_idx, anim_type)) in node_channels.iter().enumerate() {
            let sampler = &animation.samplers[*sampler_idx];
            let sample_result = animation_instance.samples[*sampler_idx].sample(
                time_delta,
                &sampler.times,
                sampler_count + 1 == node_channels.len(),
            );

            match sample_result {
                // TODO: considering a sample to be "done" may actually
                // be incorrect. GLTF might expect that the value persists (is clamped) after the
                // last animation time, as well as before the first ainimation time.
                // this doesnt cost us that much if so. it amounts to the last_quat/last_vec3 calcs
                // which is not a lot
                SampleResult::Done => continue,
                SampleResult::End => match anim_type {
                    AnimationTransformType::Rotation => {
                        rotation = Some(last_quat(&sampler.transforms.0));
                    }
                    AnimationTransformType::Translation => {
                        translation = Some(last_vec3(&sampler.transforms.0));
                    }
                    AnimationTransformType::Scale => {
                        scale = Some(last_vec3(&sampler.transforms.0));
                    }
                },
                SampleResult::Active(i) => {
                    let ratio =
                        (time_delta - sampler.times[i]) / (sampler.times[i + 1] - sampler.times[i]);
                    match anim_type {
                        AnimationTransformType::Rotation => {
                            rotation = Some(interpolate_as_quats(i, ratio, &sampler.transforms.0));
                        }
                        AnimationTransformType::Translation => {
                            translation =
                                Some(interpolate_as_vec3(i, ratio, &sampler.transforms.0));
                        }
                        AnimationTransformType::Scale => {
                            scale = Some(interpolate_as_vec3(i, ratio, &sampler.transforms.0));
                        }
                    }
                }
            }
        }
    }
    let node_transform = {
        let translation: Vector3<f32> = translation.unwrap_or_else(|| {
            let NodeTransforms::Translation(t) = node.transform_components[0] else {
                unreachable!()
            };
            t
        });
        let rotation: Quaternion<f32> = rotation.unwrap_or_else(|| {
            let NodeTransforms::Rotation(r) = node.transform_components[1] else {
                unreachable!()
            };
            r
        });
        let scale: Vector3<f32> = scale.unwrap_or_else(|| {
            let NodeTransforms::Scale(s) = node.transform_components[2] else {
                unreachable!()
            };
            s
        });
        let r: cgmath::Matrix3<f32> = rotation.into();
        cgmath::Matrix4::new(
            r.x.x * scale.x,
            r.x.y * scale.x,
            r.x.z * scale.x,
            0.0,
            r.y.x * scale.y,
            r.y.y * scale.y,
            r.y.z * scale.y,
            0.0,
            r.z.x * scale.z,
            r.z.y * scale.z,
            r.z.z * scale.z,
            0.0,
            translation.x,
            translation.y,
            translation.z,
            1.0,
        )
    };

    let global = base_transform * node_transform;

    match node.node_type {
        NodeType::Mesh(mesh_id) => {
            animation_instance.buffer[buffer_slot_map[mesh_id]] = global.into();
        }
        NodeType::Node => {
            //
        }
    }

    for child_node in node.children.iter() {
        get_animation_data_for_node(
            child_node,
            &global,
            time_delta,
            animation,
            animation_instance,
            buffer_slot_map,
        );
    }
}

impl Animation for GltfAnimation {
    fn count(&self) -> usize {
        todo!()
    }
    fn get_animation_frame(
        &self,
        time_delta: f32,
        animation_instance: &mut crate::world::instance_manager::AnimationInstance,
        buffer_slot_map: &Vec<usize>,
    ) {
        for node in self.root_nodes.iter() {
            get_animation_data_for_node(
                node,
                &cgmath::Matrix4::<f32>::identity(),
                time_delta,
                &self,
                animation_instance,
                buffer_slot_map,
            );
        }
    }

    fn init_samples(&self) -> Vec<crate::animation::animation::AnimationSample> {
        let mut samples = Vec::with_capacity(self.samplers.len());
        for sampler in &self.samplers {
            samples.push(AnimationSample::init(&sampler.times));
        }
        samples
    }

    #[cfg(test)]
    fn get_channels_and_samplers(&self) -> (&AnimationChannels, &Vec<AnimationSampler>) {
        (&self.channels, &self.samplers)
    }
}

fn interpolate_as_quats(cursor: usize, ratio: f32, floats: &[f32]) -> cgmath::Quaternion<f32> {
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

fn last_quat(floats: &[f32]) -> cgmath::Quaternion<f32> {
    let last: &[f32] = &floats[floats.len() - 4..floats.len()];
    cgmath::Quaternion::new(last[3], last[0], last[1], last[2])
}
fn last_vec3(floats: &[f32]) -> cgmath::Vector3<f32> {
    let last: &[f32] = &floats[floats.len() - 3..floats.len()];
    cgmath::Vector3::new(last[0], last[1], last[2])
}
