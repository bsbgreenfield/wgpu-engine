use std::collections::HashMap;

use crate::world::{
    components::ComponentData,
    entity_manager::EntityHandle,
    instance_arena::{InstanceArena, InstanceHandle},
};

pub struct InstanceManager {
    arena: InstanceArena,
    entity_to_instance: HashMap<EntityHandle, Vec<InstanceHandle>>,
}

impl InstanceManager {
    pub(super) fn new() -> Self {
        Self {
            arena: InstanceArena::new(),
            entity_to_instance: HashMap::new(),
        }
    }
    pub(super) fn spawn(
        &mut self,
        entity_handle: EntityHandle,
        data: Vec<Box<dyn ComponentData>>,
    ) -> &Vec<InstanceHandle> {
        let instance_handle = self.arena.insert(data);
        if self.entity_to_instance.contains_key(&entity_handle) {
            self.entity_to_instance
                .entry(entity_handle)
                .and_modify(|instances| instances.push(instance_handle));
        } else {
            self.entity_to_instance
                .insert(entity_handle, vec![instance_handle]);
        }

        self.entity_to_instance.get(&entity_handle).unwrap()
    }

    pub fn entity_of(&self, instance_handle: &InstanceHandle) -> EntityHandle {
        todo!()
    }
}
