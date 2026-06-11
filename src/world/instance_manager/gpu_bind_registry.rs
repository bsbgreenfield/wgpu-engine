use std::collections::HashMap;

use crate::{
    app::renderer::{PrototypeHandle, renderer::GPUInstanceHandle},
    world::{entity_manager::EntityHandle, instance_manager::InstanceHandle},
};

#[derive(Default)]
pub(super) struct GPUBindRegistry {
    pub(super) next_prototype: u16,
    pub(super) registered_prototypes: HashMap<EntityHandle, PrototypeHandle>,
    pub(super) registered_instances: HashMap<GPUInstanceHandle, InstanceHandle>,
}

impl GPUBindRegistry {
    pub(super) fn gen_prototype(&mut self, entity_handle: EntityHandle) -> PrototypeHandle {
        let prototype = PrototypeHandle(self.next_prototype);
        self.next_prototype += 1;
        self.registered_prototypes
            .insert(entity_handle, prototype.clone());

        prototype
    }
}
