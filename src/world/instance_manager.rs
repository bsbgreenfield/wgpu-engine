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

    fn insert_self(
        self,
        manager: &mut InstanceManager,
        entity_handle: EntityHandle,
    ) -> InstanceHandle;

    fn despawn_self(manager: &mut InstanceManager, handle: InstanceHandle);

    fn get_state<'a, C: ComponentData + 'a>(
        manager: &'a InstanceManager,
        handle: InstanceHandle,
    ) -> Option<&'a impl ComponentData>;
}

#[derive(Debug, Clone, Copy)]
pub enum ArchetypeId {
    Position = 0,
}

pub trait ArchetypeTable {
    type A: Archetype;

    fn new() -> Self;

    fn insert(
        &mut self,
        data: Self::A,
        globla_id: u16,
        entity_handle: EntityHandle,
    ) -> InstanceHandle;

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
    fn insert_self(
        self,
        manager: &mut InstanceManager,
        entity_handle: EntityHandle,
    ) -> InstanceHandle {
        let global_id = manager.gen_global_id();
        manager.pos.insert(self, global_id, entity_handle)
    }

    fn despawn_self(manager: &mut InstanceManager, handle: InstanceHandle) {
        manager.pos.remove(handle);
    }

    fn get_state<'a, C: ComponentData + 'a>(
        manager: &'a InstanceManager,
        handle: InstanceHandle,
    ) -> Option<&'a impl ComponentData> {
        manager.pos.resolve::<C>(handle)
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

    fn new() -> Self {
        Self {
            positions: Vec::new(),
            arena: InstanceArenaNew::new(),
        }
    }

    fn insert(
        &mut self,
        data: APosition,
        global_id: u16,
        entity_handle: EntityHandle,
    ) -> InstanceHandle {
        self.positions.push(data.position);
        self.arena.insert(global_id, entity_handle)
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
#[derive(Clone, Debug)]
pub struct InstanceHandle {
    pub global_id: u16,
    pub entity_handle: EntityHandle,
    pub instance_id: u16,
    pub generation: u16,
}

pub struct InstanceManager {
    free_ids: Vec<u16>,
    pub(super) next_id: u16,
    entity_to_instance: HashMap<EntityHandle, Vec<InstanceHandle>>,
    pub(super) pos: APositionTable,
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
            free_ids: Vec::new(),
            entity_to_instance: HashMap::new(),
            pos: APositionTable::new(),
        }
    }

    fn gen_global_id(&mut self) -> u16 {
        if let Some(free) = self.free_ids.pop() {
            free
        } else {
            self.next_id += 1;
            self.next_id - 1
        }
    }

    pub(super) fn spawn<A: Archetype>(
        &mut self,
        entity_handle: EntityHandle,
        data: A,
    ) -> &Vec<InstanceHandle> {
        let instance_handle = data.insert_self(self, entity_handle);

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

    pub fn despawn<A: Archetype>(&mut self, handle: InstanceHandle) {
        A::despawn_self(self, handle);
    }

    pub fn get_state<'a, A: Archetype + 'a, C: ComponentData + 'a>(
        &'a self,
        handle: InstanceHandle,
    ) -> Option<&'a impl ComponentData> {
        A::get_state::<C>(self, handle)
    }
}
