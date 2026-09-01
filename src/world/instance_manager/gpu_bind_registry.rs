use std::collections::HashMap;

use crate::{
    common::entity::EntityHandle,
    renderer::{GPUInstanceHandle, PrototypeHandle},
    world::{WorldUpdateError, instance_manager::InstanceHandle},
};

#[derive(Default)]
pub(super) struct GPUBindRegistry {
    pub(super) next_prototype: u32,
    pub(super) registered_prototypes: HashMap<EntityHandle, PrototypeHandle>,
    pub(super) registered_instances: HashMap<GPUInstanceHandle, InstanceHandle>,
}

impl GPUBindRegistry {
    pub(super) fn gen_prototype(&mut self, entity_handle: EntityHandle) -> PrototypeHandle {
        let prototype =
            if let Some(prototype_handle) = self.registered_prototypes.get(&entity_handle) {
                *prototype_handle
            } else {
                let p = PrototypeHandle::new(self.next_prototype);
                self.next_prototype += 1;
                p
            };
        self.registered_prototypes.insert(entity_handle, prototype);

        prototype
    }

    pub(super) fn unregister(
        &mut self,
        instance_handle: &InstanceHandle,
    ) -> Result<GPUInstanceHandle, WorldUpdateError> {
        let key = self
            .registered_instances
            .iter()
            .find(|(_key, val)| val == &instance_handle)
            .ok_or(WorldUpdateError::InstancceNotFound(instance_handle.clone()))?
            .0
            .clone();
        self.registered_instances
            .remove(&key)
            .ok_or(WorldUpdateError::InstancceNotFound(instance_handle.clone()))?;
        Ok(key)
    }
}
