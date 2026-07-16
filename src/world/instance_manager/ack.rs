use crate::{
    common::{
        entity::{EntityHandle, PrototypeHandle},
        instance::{GPUInstanceHandle, InstanceHandle},
    },
    world::instance_manager::{
        ArchetypeId, archetype_table::ArchetypeTable, instance_manager::InstanceManager,
    },
};

impl InstanceManager {
    pub fn register_prototype(
        &mut self,
        entity_handle: EntityHandle,
        prototype_handle: PrototypeHandle,
    ) {
        self.gpu_bind_registry
            .registered_prototypes
            .insert(entity_handle, prototype_handle);
    }
    pub fn add_record_index(
        &mut self,
        instance_handle: &InstanceHandle,
        record_index: u32,
        gpu_instance_handle: GPUInstanceHandle,
    ) {
        self.gpu_bind_registry
            .registered_instances
            .insert(gpu_instance_handle, instance_handle.clone());
        match instance_handle.archetype {
            ArchetypeId::Position => {
                self.pos.write_record_index(instance_handle, record_index);
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
