use crate::{
    common::{entity::EntityHandle, instance::InstanceHandle},
    util::types::GlobalTransform,
    world::instance_manager::{archetype_table::ArchetypeTable, instance_manager::InstanceManager},
};

pub trait ArchetypeIdent {
    const ARCHETYPE_ID: ArchetypeId;
}

pub trait Archetype {
    fn insert_self(
        self: Box<Self>,
        manager: &mut InstanceManager,
        entity_handle: &EntityHandle,
    ) -> InstanceHandle;
}
pub struct APosition {
    pub position: GlobalTransform,
}

impl ArchetypeIdent for APosition {
    const ARCHETYPE_ID: ArchetypeId = ArchetypeId::Position;
}
pub struct APositionRef<'a> {
    pub position: &'a GlobalTransform,
}
impl Archetype for APosition {
    fn insert_self(
        self: Box<Self>,
        manager: &mut InstanceManager,
        entity_handle: &EntityHandle,
    ) -> InstanceHandle {
        manager.pos.insert(*self, *entity_handle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchetypeId {
    Position = 0,
}
impl TryFrom<u16> for ArchetypeId {
    type Error = ();
    fn try_from(v: u16) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Position),
            _ => Err(()),
        }
    }
}
