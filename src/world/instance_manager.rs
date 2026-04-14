use std::collections::HashMap;

use crate::{
    app::renderer::renderer::InstanceDataCollector,
    util::types::GlobalTransform,
    world::{
        components::{ComponentData, ComponentDataType},
        entity_manager::EntityHandle,
        index_arena::InstanceArenaNew,
    },
};

pub trait ArchetypeIdent {
    const ARCHETYPE_ID: ArchetypeId;
}

pub trait Archetype {
    fn insert_self(
        self,
        manager: &mut InstanceManager,
        entity_handle: EntityHandle,
    ) -> InstanceHandle;
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
        self,
        manager: &mut InstanceManager,
        entity_handle: EntityHandle,
    ) -> InstanceHandle {
        let global_id = manager.gen_global_id();
        manager.pos.insert(self, global_id, entity_handle)
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
        collector.offset_map.a_postion_offset = offset;
        collector.gt_len += self.positions.len();
        collector.global_transforms.push(&self.positions[..]);
    }

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
    pub archetype: ArchetypeId,
    pub entity_handle: EntityHandle,
    pub instance_id: u16,
    pub generation: u16,
}

pub struct InstanceManager {
    free_ids: Vec<u16>,
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
        // TODO
    }
}
