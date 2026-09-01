#[cfg(test)]
use std::{collections::HashMap, sync::Arc};

use crate::world::instance_manager::instance_manager::InstanceManager;
#[cfg(test)]
use crate::{
    animation::{Animation, AnimationInstance, EntityAnimations},
    common::{entity::EntityHandle, instance::InstanceHandle},
    renderer::{GPUInstanceHandle, PrototypeHandle},
    util::types::GlobalTransform,
    world::world::RenderGroup,
};

impl InstanceManager {
    #[cfg(test)]
    pub fn run_animations(&mut self, time_delta: f32) {
        self.animation_controller.run_animations(time_delta);
    }

    #[cfg(test)]
    pub fn get_buffer_slot_map(&self, instance_idx: usize) -> &Vec<usize> {
        let a = &self.animation_controller.active_animations[instance_idx];
        let entity_anim = self
            .animation_controller
            .registered_animations
            .get(&a.instance_handle.entity_handle)
            .unwrap();

        &entity_anim.mesh_slot_map
    }
    #[cfg(test)]
    pub fn get_registered_prototypes(&self) -> &HashMap<EntityHandle, PrototypeHandle> {
        &self.gpu_bind_registry.registered_prototypes
    }
    #[cfg(test)]
    pub fn get_registered_instances(&self) -> &HashMap<GPUInstanceHandle, InstanceHandle> {
        &self.gpu_bind_registry.registered_instances
    }

    #[cfg(test)]
    pub fn assert_animation_exists(&self, instance_handle: &InstanceHandle) {
        assert!(
            self.animation_controller
                .registered_animations
                .contains_key(&instance_handle.entity_handle)
        )
    }

    #[cfg(test)]
    pub fn get_joint_slot_map(&self, entity_handle: &EntityHandle) -> &Vec<usize> {
        &self
            .animation_controller
            .registered_animations
            .get(entity_handle)
            .expect("entity must be registered")
            .skin_offset_map
    }

    #[cfg(test)]
    pub fn get_active_animations(&self) -> &[AnimationInstance] {
        &self.animation_controller.active_animations
    }

    #[cfg(test)]
    pub fn get_all_instances(&self) -> Vec<InstanceHandle> {
        self.gpu_bind_registry
            .registered_instances
            .values()
            .cloned()
            .collect()
    }

    #[cfg(test)]
    pub fn get_animation_ref(
        &self,
        entity_handle: &EntityHandle,
        index: usize,
    ) -> &Arc<dyn Animation> {
        &self
            .animation_controller
            .registered_animations
            .get(entity_handle)
            .unwrap()
            .animation[index]
    }

    #[cfg(test)]
    pub fn get_entity_animation(&self, entity_handle: &EntityHandle) -> Option<&EntityAnimations> {
        self.animation_controller
            .registered_animations
            .get(entity_handle)
    }

    #[cfg(test)]
    pub fn get_pos_table_positions(&self) -> Vec<GlobalTransform> {
        self.pos.get_positions()
    }
    #[cfg(test)]
    pub fn get_pos_table_handles(&self) -> Vec<InstanceHandle> {
        self.pos.arena.handles.clone()
    }

    #[cfg(test)]
    pub(crate) fn get_groups(&self) -> &Vec<RenderGroup> {
        &self.render_groups
    }
}
