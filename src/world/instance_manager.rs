use std::collections::HashMap;

use crate::{
    util::types::GlobalTransform,
    world::{
        entity_manager::EntityHandle,
        index_arena_new::{Archetype, ArchetypeId, InstanceArenaNew},
        instance_arena::InstanceHandle,
    },
};

pub trait ArchetypeTable {
    type A: Archetype;

    fn new() -> Self;

    fn insert(&mut self, data: Self::A, handle: InstanceHandle) -> u16;
}

struct APosition {
    len: usize,
    position: GlobalTransform,
    instances: Vec<u32>,
}

impl Archetype for APosition {
    fn id(&self) -> ArchetypeId {
        ArchetypeId::Position
    }
    fn insert_self(self, manager: &mut InstanceManager, handle: InstanceHandle) -> u16 {
        manager.pos.insert(self, handle)
    }
}

struct APositionTable {
    positions: Vec<GlobalTransform>,
    instances: Vec<InstanceHandle>,
}

impl ArchetypeTable for APositionTable {
    type A = APosition;

    fn new() -> Self {
        Self {
            positions: Vec::new(),
            instances: Vec::new(),
        }
    }

    fn insert(&mut self, data: APosition, handle: InstanceHandle) -> u16 {
        self.positions.push(data.position);
        self.instances.push(handle);

        (self.positions.len() - 1) as u16
    }
}

pub struct InstanceManager {
    arena: InstanceArenaNew,
    entity_to_instance: HashMap<EntityHandle, Vec<InstanceHandle>>,
    pos: APositionTable,
}

impl InstanceManager {
    pub(super) fn new() -> Self {
        Self {
            arena: InstanceArenaNew::default(),
            entity_to_instance: HashMap::new(),
            pos: APositionTable::new(),
        }
    }
    pub(super) fn spawn<A: Archetype>(
        &mut self,
        entity_handle: EntityHandle,
        data: A,
    ) -> &Vec<InstanceHandle> {
        let instance_handle = self.arena.insert(&data);
        let index = data.insert_self(self, instance_handle.clone());
        self.arena.set_index(index, instance_handle.instance_id);

        if self.entity_to_instance.contains_key(&entity_handle) {
            self.entity_to_instance
                .entry(entity_handle)
                .and_modify(|instances| instances.push(instance_handle));
        } else {
            self.entity_to_instance
                .insert(entity_handle, vec![instance_handle]);
        }

        self.entity_to_instance.get(&entity_handle).unwrap()
    }

    pub fn despawn(&mut self, handle: InstanceHandle) {}

    pub fn entity_of(&self, instance_handle: &InstanceHandle) -> EntityHandle {
        todo!()
    }
}
