use crate::{
    common::instance::InstanceHandle,
    renderer::GPUInstanceHandle,
    world::{
        InstanceResidency,
        instance_manager::{
            ArchetypeId, archetype_table::ArchetypeTable, instance_manager::InstanceManager,
        },
    },
};

impl InstanceManager {
    pub fn add_record_index(
        &mut self,
        instance_handle: &InstanceHandle,
        residency: InstanceResidency,
        gpu_instance_handle: GPUInstanceHandle,
    ) {
        self.gpu_bind_registry
            .registered_instances
            .insert(gpu_instance_handle, instance_handle.clone());
        self.gpu_bind_registry
            .active_bindings
            .insert(residency.bind_key);
        match instance_handle.archetype {
            ArchetypeId::Position => {
                self.pos.write_record_index(instance_handle, residency);
            }
        }

        // animation
        if let Some(entity_animations) = self
            .animation_controller
            .registered_animations
            .get_mut(&instance_handle.entity_handle)
        {
            entity_animations.gpu_instance_handle = Some(gpu_instance_handle);
        }
    }
}
