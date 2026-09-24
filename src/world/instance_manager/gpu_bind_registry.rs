use std::collections::HashMap;

use crate::{
    renderer::GPUInstanceHandle,
    world::{WorldUpdateError, instance_manager::InstanceHandle},
};

#[derive(Default)]
pub(super) struct GPUBindRegistry {
    pub(super) registered_instances: HashMap<InstanceHandle, GPUInstanceHandle>,
}

impl GPUBindRegistry {
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
