use crate::{
    common::entity::{EntityHandle, PrototypeHandle},
    world::instance_manager::ArchetypeId,
};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct InstanceHandle {
    pub archetype: ArchetypeId,
    pub entity_handle: EntityHandle,
    pub instance_id: u16,
    pub generation: u16,
}
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GPUInstanceHandle {
    pub prototype: PrototypeHandle,
    pub instance_id: u32,
}
