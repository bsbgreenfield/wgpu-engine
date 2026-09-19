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
        record_offset: u32,
        bind_key: u32,
        gpu_instance_handle: GPUInstanceHandle,
    ) {
        self.gpu_bind_registry
            .registered_instances
            .insert(instance_handle.clone(), gpu_instance_handle);
        self.gpu_bind_registry.active_bindings.insert(bind_key);
        let residency = InstanceResidency {
            bind_key,
            record_index: record_offset,
            group_id: self.sparse_entity_group[instance_handle.entity_handle.0 as usize] as u64,
        };
        match instance_handle.archetype {
            ArchetypeId::Position => {
                self.pos.write_record_index(instance_handle, residency);
            }
        }

        // animation
        //if let Some(entity_animations) = self
        //    .animation_controller
        //    .registered_animations
        //    .get_mut(&instance_handle.entity_handle)
        //{
        //    entity_animations.gpu_instance_handle = Some(gpu_instance_handle);
        //}
    }
}
