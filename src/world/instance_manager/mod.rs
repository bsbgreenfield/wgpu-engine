use crate::{
    app::renderer::renderer::GPUInstanceHandle,
    util::types::{GlobalTransform, InstanceOffset},
    world::{
        RenderKey, entity_manager::EntityHandle,
        instance_manager::instance_manager::InstanceManager,
    },
};

mod animation_controller;
pub mod archetype_table;
mod draw_palette;
mod gpu_bind_registry;
mod instance_arena;
pub(super) mod instance_manager;

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

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct InstanceHandle {
    pub archetype: ArchetypeId,
    pub entity_handle: EntityHandle,
    pub instance_id: u16,
    pub generation: u16,
}

impl RenderKey for InstanceHandle {
    fn as_key(&self) -> u64 {
        let i = self.instance_id as u64;
        let e = (self.entity_handle.0 as u64) << 16;
        let a = (self.archetype as u64) << 32;
        let g = (self.generation as u64) << 48;
        i | e | a | g
    }

    fn from_key(key: u64) -> Self {
        let instance = (key & 0xFFFF) as u16;
        let entity = ((key >> 16) & 0xFFFF) as u16;
        let archetype = ((key >> 32) & 0xFFFF) as u16;
        let generation = ((key >> 48) & 0xFFFF) as u16;

        Self {
            archetype: ArchetypeId::try_from(archetype).expect("invalid archetype in key"),
            entity_handle: EntityHandle(entity),
            generation,
            instance_id: instance,
        }
    }
}

#[cfg(test)]
impl InstanceHandle {
    pub fn mock(
        archetype: ArchetypeId,
        entity_handle: EntityHandle,
        instance_id: u16,
        generation: u16,
    ) -> Self {
        Self {
            archetype,
            entity_handle,
            instance_id,
            generation,
        }
    }
}

#[derive(Debug)]
pub struct AnimationUpdate<'frame> {
    pub gpu_handle: GPUInstanceHandle,
    pub transforms: &'frame [u8],
}

#[derive(Debug, Default)]
pub struct RenderFrame<'frame> {
    pub global_transforms: &'frame [GlobalTransform],
    pub indirection_list: &'frame [u32],
    pub rigid_animation_data: Vec<AnimationUpdate<'frame>>,
    pub joint_animation_data: Vec<AnimationUpdate<'frame>>,
}

#[derive(Debug)]
pub struct InstanceGPUBindings {
    pub lt_offset: u32,
    pub joint_offset: Option<u32>,
}
