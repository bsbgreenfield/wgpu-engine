use std::collections::HashMap;

use crate::{
    app::renderer::{PrototypeHandle, renderer::GPUInstanceHandle},
    world::{entity_manager::EntityHandle, instance_manager::InstanceHandle},
};

#[derive(Default)]
pub(super) struct GPUBindRegistry {
    pub(super) registered_prototypes: HashMap<EntityHandle, PrototypeHandle>,
    pub(super) registered_instances: HashMap<GPUInstanceHandle, InstanceHandle>,
}
