use crate::{
    util::types::GlobalTransform,
    world::{
        entity_manager::EntityHandle,
        instance_manager::{
            Archetype, ArchetypeId, ArchetypeIdent, InstanceHandle, RenderFrame,
            instance_arena::InstanceArena, instance_manager::InstanceManager,
        },
    },
};

pub(super) trait ArchetypeTable {
    type A: Archetype;

    fn new() -> Self;

    fn insert(&mut self, data: Self::A, entity_handle: EntityHandle) -> InstanceHandle;

    fn remove(&mut self, handle: InstanceHandle);

    fn collect<'a>(&'a self, collector: &mut RenderFrame<'a>);
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
        entity_handle: &EntityHandle,
    ) -> InstanceHandle {
        manager.pos.insert(*self, *entity_handle)
    }
}

pub(super) struct APositionTable {
    pub(super) positions: Vec<GlobalTransform>,
    pub(super) arena: InstanceArena<APosition>,
}
#[cfg(test)]
impl APositionTable {
    pub fn get_positions(&self) -> Vec<GlobalTransform> {
        self.positions.clone()
    }
}

impl ArchetypeTable for APositionTable {
    type A = APosition;

    fn collect<'a>(&'a self, render_frame: &mut RenderFrame<'a>) {
        if !self.positions.is_empty() {
            render_frame
                .global_transforms
                .push(bytemuck::cast_slice(&self.positions[..]));
        }
    }

    fn new() -> Self {
        Self {
            positions: Vec::new(),
            arena: InstanceArena::new(),
        }
    }

    fn insert(&mut self, data: APosition, entity_handle: EntityHandle) -> InstanceHandle {
        self.positions.push(data.position);
        self.arena.insert(entity_handle)
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
