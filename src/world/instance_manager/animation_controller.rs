use std::collections::HashMap;

use time::{Duration, ext::InstantExt};

use crate::{
    animation::animation::{AnimationInstance, EntityAnimations},
    util::types::Mat4F32,
    world::{
        entity_manager::EntityHandle,
        instance_manager::{AnimationUpdate, InstanceHandle, RenderFrame},
    },
};

#[derive(Default)]
pub struct AnimationController {
    pub(super) registered_animations: HashMap<EntityHandle, EntityAnimations>,
    pub active_animations: Vec<AnimationInstance>,
}

impl AnimationController {
    pub(super) fn clear_animation_for(&mut self, instance_handle: &InstanceHandle) {
        for anim in self.active_animations.iter_mut() {
            if &anim.instance_handle == instance_handle {
                anim.complete = true;
            }
        }
    }
    /// time offset is unsafe: only use if you are sure the offset is a valid value for the animation, or if
    /// the animation is repeating
    pub(super) fn activate_animations(
        &mut self,
        instance_handle: &InstanceHandle,
        anim_idx: usize,
        time_offset: Option<f32>,
    ) -> Option<()> {
        let entity_animation = self
            .registered_animations
            .get(&instance_handle.entity_handle)?;

        let mesh_buffer: Vec<Mat4F32> = entity_animation
            .local_transforms
            .iter()
            .map(|lt| **lt)
            .collect();

        self.active_animations.push(AnimationInstance {
            complete: false,
            samples: entity_animation.animation[anim_idx].init_samples(),
            mesh_buffer,
            joint_buffer: entity_animation.joint_transforms.clone(),
            animation_idx: anim_idx,
            start_time: std::time::Instant::now()
                .add_signed(Duration::milliseconds(time_offset.unwrap_or(0.0) as i64)),
            instance_handle: instance_handle.clone(),
        });
        Some(())
    }

    pub(super) fn update(&mut self) {
        let mut anim_count = self.active_animations.len();
        let mut cursor = 0;
        'outer: while cursor < anim_count {
            'inner: loop {
                if self.active_animations[cursor].complete {
                    self.active_animations.swap_remove(cursor);
                    anim_count -= 1;
                    if cursor >= anim_count {
                        break 'outer;
                    }
                } else {
                    break 'inner;
                }
            }
            let active_animation = &mut self.active_animations[cursor];
            let entity_animation = self
                .registered_animations
                .get(&active_animation.instance_handle.entity_handle)
                .unwrap();
            let animation = &entity_animation.animation[active_animation.animation_idx];

            let now = std::time::Instant::now();
            let time_delta: f32 = (now - active_animation.start_time).as_secs_f32();
            active_animation.complete = animation.get_animation_frame(
                time_delta,
                active_animation,
                &entity_animation.mesh_slot_map,
                &entity_animation.skin_offset_map,
            );
            cursor += 1;
        }
    }

    pub(super) fn prepare_animation_frame<'frame>(
        &'frame self,
        render_frame: &mut RenderFrame<'frame>,
    ) {
        for animation_instance in self.active_animations.iter() {
            let gpu_handle = self
                .registered_animations
                .get(&animation_instance.instance_handle.entity_handle)
                .unwrap()
                .gpu_instance_handle
                .unwrap();
            render_frame.rigid_animation_data.push(AnimationUpdate {
                gpu_handle,
                transforms: bytemuck::cast_slice(&animation_instance.mesh_buffer),
            });

            if !animation_instance.joint_buffer.is_empty() {
                render_frame.joint_animation_data.push(AnimationUpdate {
                    gpu_handle,
                    transforms: bytemuck::cast_slice(&animation_instance.joint_buffer),
                });
            }
        }
    }

    #[cfg(test)]
    pub(super) fn run_animations(&mut self, time_delta: f32) {
        for active_animation in self.active_animations.iter_mut() {
            let entity_animation = self
                .registered_animations
                .get(&active_animation.instance_handle.entity_handle)
                .unwrap();
            let animation = &entity_animation.animation[active_animation.animation_idx];

            animation.get_animation_frame(
                time_delta,
                active_animation,
                &entity_animation.mesh_slot_map,
                &entity_animation.skin_offset_map,
            );
        }
    }
}
