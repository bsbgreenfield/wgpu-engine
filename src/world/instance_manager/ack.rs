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
    pub fn ack_instance_spawn(
        &mut self,
        instance_handle: &InstanceHandle,
        gpu_instance_handle: GPUInstanceHandle,
    ) {
        self.gpu_bind_registry
            .registered_instances
            .insert(instance_handle.clone(), gpu_instance_handle);
        let entity_group_slot = self.sparse_entity_group[instance_handle.entity_handle.0 as usize];
        debug_assert_ne!(
            entity_group_slot,
            usize::MAX,
            "spawn ack for {:?} whose render group is released",
            instance_handle.entity_handle
        );
        let residency = InstanceResidency {
            bind_key: gpu_instance_handle.bind_id as u32,
            record_index: gpu_instance_handle.instance_id,
            group_id: entity_group_slot as u64,
        };
        match instance_handle.archetype {
            ArchetypeId::Position => {
                self.pos.write_record_index(instance_handle, residency);
            }
        }
    }

    //    pub fn add_record_index(
    //        &mut self,
    //        instance_handle: &InstanceHandle,
    //        record_offset: u32,
    //        bind_key: u32,
    //        gpu_instance_handle: GPUInstanceHandle,
    //    ) {
    //        self.gpu_bind_registry
    //            .registered_instances
    //            .insert(instance_handle.clone(), gpu_instance_handle);
    //        self.gpu_bind_registry.active_bindings.insert(bind_key);
    //        let residency = InstanceResidency {
    //            bind_key,
    //            record_index: record_offset,
    //            group_id: self.sparse_entity_group[instance_handle.entity_handle.0 as usize] as u64,
    //        };
    //        match instance_handle.archetype {
    //            ArchetypeId::Position => {
    //                self.pos.write_record_index(instance_handle, residency);
    //            }
    //        }
    //
    // animation
    //if let Some(entity_animations) = self
    //    .animation_controller
    //    .registered_animations
    //    .get_mut(&instance_handle.entity_handle)
    //{
    //    entity_animations.gpu_instance_handle = Some(gpu_instance_handle);
    //}
    //   }
}
