use crate::{
    common::entity::EntityHandle,
    util::types::GlobalTransform,
    world::instance_manager::{
        InstanceHandle,
        archetypes::{APosition, APositionRef, Archetype},
        instance_arena::InstanceArena,
    },
};

pub trait ArchetypeTable {
    type A: Archetype;
    type Ref<'a>
    where
        Self: 'a;

    fn new() -> Self;

    fn insert(&mut self, data: Self::A, entity_handle: EntityHandle) -> InstanceHandle;

    fn remove(&mut self, handle: InstanceHandle);

    fn write_record_index(&mut self, handle: &InstanceHandle, index: u32);

    fn query<'a>(&'a self, handle: &InstanceHandle) -> Option<Self::Ref<'a>>;
}

pub struct APositionTable {
    pub(super) positions: Vec<GlobalTransform>,
    pub(super) arena: InstanceArena<APosition>,
    pub(super) record_indices: Vec<u32>,
}
#[cfg(test)]
impl APositionTable {
    pub fn get_positions(&self) -> Vec<GlobalTransform> {
        self.positions.clone()
    }
}

impl ArchetypeTable for APositionTable {
    type A = APosition;
    type Ref<'a>
        = APositionRef<'a>
    where
        Self: 'a;

    fn query<'a>(&'a self, handle: &InstanceHandle) -> Option<Self::Ref<'a>> {
        match self.arena.resolve(handle) {
            Some(dense) => Some(APositionRef {
                position: &self.positions[dense],
            }),
            None => None,
        }
    }

    fn write_record_index(&mut self, handle: &InstanceHandle, index: u32) {
        let dense_idx = self
            .arena
            .resolve(handle)
            .expect("cannot resolve the dense idx of this instance");
        self.record_indices[dense_idx] = index;
    }

    fn new() -> Self {
        Self {
            positions: Vec::new(),
            arena: InstanceArena::new(),
            record_indices: Vec::new(),
        }
    }

    fn insert(&mut self, data: APosition, entity_handle: EntityHandle) -> InstanceHandle {
        self.positions.push(data.position);
        self.record_indices.push(0); // allocate a dummy value (dangerous to be 0?)
        self.arena.insert(entity_handle)
    }

    fn remove(&mut self, handle: InstanceHandle) {
        // TODO: draw data becomes orphaned once the instance is gone, so we need a defrag algorithm
        // to reduce memory leaks here. Not as much of a concern until im running at scale
        if let Some(idx_of_goner) = self.arena.remove(handle) {
            let last = self.positions.len() - 1;
            self.positions.swap(idx_of_goner, last);
            self.record_indices.swap(idx_of_goner, last);
        }
        self.positions.pop();
        self.record_indices.pop();
    }
}
