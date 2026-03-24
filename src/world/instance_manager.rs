use std::collections::HashMap;

use crate::{
    util::types::GlobalTransform,
    world::{
        components::{ComponentData, ComponentDataType},
        entity_manager::EntityHandle,
        index_arena::InstanceArenaNew,
    },
};

pub trait Archetype {
    fn id() -> ArchetypeId;

    fn insert_self(self, manager: &mut InstanceManager) -> InstanceHandle;
}

#[derive(Debug, Clone, Copy)]
pub enum ArchetypeId {
    Position = 0,
}

pub trait ArchetypeTable {
    type A: Archetype;

    fn new() -> Self;

    fn insert(&mut self, data: Self::A) -> InstanceHandle;

    fn remove(&mut self, handle: InstanceHandle);

    fn resolve<C: ComponentData>(&self, handle: InstanceHandle) -> Option<&impl ComponentData>;
}

pub struct APosition {
    pub position: GlobalTransform,
}

impl Archetype for APosition {
    fn id() -> ArchetypeId {
        ArchetypeId::Position
    }
    fn insert_self(self, manager: &mut InstanceManager) -> InstanceHandle {
        manager.pos.insert(self)
    }
}

struct APositionTable {
    positions: Vec<GlobalTransform>,
    arena: InstanceArenaNew<APosition>,
}

impl ArchetypeTable for APositionTable {
    type A = APosition;

    fn new() -> Self {
        Self {
            positions: Vec::new(),
            arena: InstanceArenaNew::new(),
        }
    }

    fn insert(&mut self, data: APosition) -> InstanceHandle {
        self.positions.push(data.position);
        self.arena.insert()
    }

    fn remove(&mut self, handle: InstanceHandle) {
        let last = self.positions.len() - 1;
        if let Some(idx_of_goner) = self.arena.remove(handle) {
            self.positions.swap(idx_of_goner, last);
        } else {
            self.positions.pop();
        }
    }

    fn resolve<C>(&self, handle: InstanceHandle) -> Option<&impl ComponentData>
    where
        C: ComponentData,
    {
        if let Some(index) = self.arena.resolve(handle) {
            match C::get_data_type() {
                ComponentDataType::PhysicalPosition => Some(&self.positions[index]),
                _ => panic!(),
            }
        } else {
            return None;
        }
    }
}
#[derive(Clone)]
pub struct InstanceHandle {
    pub archetype_id: ArchetypeId,
    pub instance_id: u16,
    pub generation: u16,
}

pub struct InstanceManager {
    entity_to_instance: HashMap<EntityHandle, Vec<InstanceHandle>>,
    pos: APositionTable,
}

impl InstanceManager {
    pub(super) fn new() -> Self {
        Self {
            entity_to_instance: HashMap::new(),
            pos: APositionTable::new(),
        }
    }
    pub(super) fn spawn<A: Archetype>(
        &mut self,
        entity_handle: EntityHandle,
        data: A,
    ) -> &Vec<InstanceHandle> {
        let instance_handle = data.insert_self(self);

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

    pub fn despawn(&mut self, handle: InstanceHandle) {
        match handle.archetype_id {
            ArchetypeId::Position => self.pos.remove(handle),
        }
    }

    pub fn get_state<'a, C: ComponentData + 'a>(
        &'a self,
        handle: InstanceHandle,
    ) -> Option<&impl ComponentData> {
        match handle.archetype_id {
            ArchetypeId::Position => self.pos.resolve::<C>(handle),
        }
    }

    pub fn entity_of(&self, instance_handle: &InstanceHandle) -> EntityHandle {
        todo!()
    }
}
