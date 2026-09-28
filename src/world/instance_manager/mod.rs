use std::sync::Arc;

use crate::{
    common::{entity::EntityHandle, instance::InstanceHandle},
    renderer::{DrawSet, GPUAllocationHandle, PrototypeHandle, RenderKey},
    util::types::{InverseBindMatrix, JointTransform, LocalTransform},
    world::instance_manager::archetypes::ArchetypeId,
};

pub(super) mod ack;
mod animation_controller;
mod archetype_table;
pub mod archetypes;
pub mod gen_draw_calls;
mod gpu_bind_registry;
mod instance_arena;
pub(super) mod instance_manager;
mod spawn;
pub mod test;

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
pub struct InstanceGPUBindings {
    pub lt_offset: u32,
    pub joint_offset: Option<u32>,
}

pub(crate) struct RenderView {
    pub alloc_handle: GPUAllocationHandle,
    pub pnujw_draws: Option<DrawSet>,
    pub pnu_draws: Option<DrawSet>,
}

#[allow(unused)]
pub(crate) struct RenderGroup {
    pub entity_handle: EntityHandle,
    views: Vec<RenderView>,
}

impl RenderGroup {
    pub(crate) fn views(&self) -> &[RenderView] {
        &self.views
    }
    pub(super) fn new(views: Vec<RenderView>, entity_handle: EntityHandle) -> Self {
        Self {
            entity_handle: entity_handle,
            views,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LocalTransforms {
    Uninit,
    OwnedShared { data: Arc<Vec<LocalTransform>> },
    OwnedCopy { data: Arc<Vec<LocalTransform>> },
    CopiedFrom { donor: InstanceHandle },
    NeedsCopy,
    SharedWith { donor: InstanceHandle },
    NeedsShared,
}

#[derive(Debug, Clone)]
pub enum JointTransforms {
    None,
    OwnedShared { data: Arc<Vec<JointTransform>> },
    OwnedCopy { data: Arc<Vec<JointTransform>> },
    NeedsCopy,
    NeedsShared,
}

#[derive(Debug, Clone)]
pub enum InverseBindMatrices {
    None,
    Owned { data: Arc<Vec<InverseBindMatrix>> },
    NeedsCopy,
    NeedsShared,
}

#[derive(Debug, Clone)]
pub struct NewInstanceData {
    pub handle: InstanceHandle,
    pub local_transforms: LocalTransforms,
    pub joint_transforms: JointTransforms,
    pub ibms: InverseBindMatrices,
    pub additional: Vec<InstanceHandle>,
}

#[derive(Debug, Clone)]
pub struct CopiedInstanceData {
    pub handles: Vec<InstanceHandle>,
    pub prototype_handle: PrototypeHandle,
    pub local_transforms: LocalTransforms,
    pub joint_transforms: JointTransforms,
}

#[derive(Debug)]
pub enum InstanceUploadData {
    New(NewInstanceData),
    Copied(CopiedInstanceData),
}
