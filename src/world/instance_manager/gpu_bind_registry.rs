use std::collections::{HashMap, HashSet};

use crate::{
    common::entity::EntityHandle,
    renderer::{GPUInstanceHandle, PrototypeHandle},
    world::{WorldUpdateError, instance_manager::InstanceHandle},
};

#[derive(Default)]
pub(super) struct GPUBindRegistry {
    pub(super) next_prototype: u32,
    pub(super) registered_prototypes: HashMap<EntityHandle, PrototypeHandle>,
    pub(super) registered_instances: HashMap<InstanceHandle, GPUInstanceHandle>,
    pub(super) active_bindings: HashSet<u32>,
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
        let res = self
            .registered_instances
            .remove(&instance_handle)
            .ok_or(WorldUpdateError::InstancceNotFound(instance_handle.clone()))?;
        Ok(res)
    }
}
