use std::collections::HashMap;

use crate::{
    app::renderer::renderer::InstanceDataCollector,
    util::types::GlobalTransform,
    world::{entity_manager::EntityHandle, index_arena::InstanceArenaNew},
};

pub trait ArchetypeIdent {
    const ARCHETYPE_ID: ArchetypeId;
}

pub trait Archetype {
    fn insert_self(
        self: Box<Self>,
        manager: &mut InstanceManager,
        entity_handle: EntityHandle,
    ) -> InstanceHandle;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchetypeId {
    Position = 0,
}

pub trait ArchetypeTable {
    type A: Archetype;

    fn new() -> Self;

    fn insert(&mut self, data: Self::A, entity_handle: EntityHandle) -> InstanceHandle;

    fn remove(&mut self, handle: InstanceHandle);

    fn collect<'a>(&'a self, collector: &mut InstanceDataCollector<'a>, offset: u16);
}

pub struct APosition {
    pub position: GlobalTransform,
}
impl ArchetypeIdent for APosition {
    const ARCHETYPE_ID: ArchetypeId = ArchetypeId::Position;
}
impl Archetype for APosition {
    fn insert_self(
        self: Box<Self>,
        manager: &mut InstanceManager,
        entity_handle: EntityHandle,
    ) -> InstanceHandle {
        manager.pos.insert(*self, entity_handle)
    }
}

pub struct APositionTable {
    pub(super) positions: Vec<GlobalTransform>,
    pub(super) arena: InstanceArenaNew<APosition>,
}
#[cfg(test)]
impl APositionTable {
    pub fn get_positions(&self) -> Vec<GlobalTransform> {
        self.positions.clone()
    }
}

impl ArchetypeTable for APositionTable {
    type A = APosition;

    fn collect<'a>(&'a self, collector: &mut InstanceDataCollector<'a>, offset: u16) {
        if !self.positions.is_empty() {
            collector.gt_len += self.positions.len();
            collector.global_transforms.push(&self.positions[..]);
            collector.offset_map.a_postion_offset = offset;
        }
    }

    fn new() -> Self {
        Self {
            positions: Vec::new(),
            arena: InstanceArenaNew::new(),
        }
    }

    fn insert(&mut self, data: APosition, entity_handle: EntityHandle) -> InstanceHandle {
        self.positions.push(data.position);
        let a = self.arena.insert(entity_handle);
        println!("{:?}", a);
        a
    }

    fn remove(&mut self, handle: InstanceHandle) {
        let last = self.positions.len() - 1;
        if let Some(idx_of_goner) = self.arena.remove(handle) {
            self.positions.swap(idx_of_goner, last);
        } else {
            self.positions.pop();
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

pub struct InstanceManager {
    pub(super) next_id: u16,
    entity_to_instance: HashMap<EntityHandle, Vec<InstanceHandle>>,
    pub pos: APositionTable,
}

impl InstanceManager {
    #[cfg(test)]
    pub fn get_pos_table(&self) -> &APositionTable {
        &self.pos
    }

    #[cfg(test)]
    pub fn get_all_instances(&self) -> Vec<InstanceHandle> {
        self.entity_to_instance
            .iter()
            .flat_map(|entry| entry.1.clone())
            .collect()
    }
    pub(super) fn new() -> Self {
        Self {
            next_id: 0,
            entity_to_instance: HashMap::new(),
            pos: APositionTable::new(),
        }
    }

    #[inline]
    pub(super) fn is_instanced(&self, entity_handle: EntityHandle) -> bool {
        self.entity_to_instance.contains_key(&entity_handle)
    }

    pub fn resolve_idx(&self, handle: &InstanceHandle) -> Option<usize> {
        match handle.archetype {
            ArchetypeId::Position => self.pos.arena.resolve(handle),
        }
    }

    pub(super) fn spawn(
        &mut self,
        entity_handle: EntityHandle,
        data: Box<dyn Archetype>,
    ) -> Vec<&InstanceHandle> {
        let instance_handle = data.insert_self(self, entity_handle);

        if self.entity_to_instance.contains_key(&entity_handle) {
            self.entity_to_instance
                .entry(entity_handle)
                .and_modify(|instances| instances.push(instance_handle));
        } else {
            self.entity_to_instance
                .insert(entity_handle, vec![instance_handle]);
        }

        // TODO: we will want to return a slice of instances if GPU instancing is enabled
        vec![
            self.entity_to_instance
                .get(&entity_handle)
                .unwrap()
                .last()
                .unwrap(),
        ]
    }

    pub fn despawn(&mut self, handle: InstanceHandle) {
        match handle.archetype {
            ArchetypeId::Position => self.pos.remove(handle),
        }
        // TODO: other tables
    }
}
