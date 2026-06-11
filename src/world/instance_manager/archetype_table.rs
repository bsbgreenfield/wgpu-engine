use crate::{
    util::types::GlobalTransform,
    world::{
        entity_manager::EntityHandle,
        instance_manager::{
            Archetype, ArchetypeId, ArchetypeIdent, InstanceHandle, RenderFrame,
            draw_palette::{DrawData, DrawPalette, PaletteChunk},
            instance_arena::InstanceArena,
            instance_manager::InstanceManager,
        },
    },
};

pub(super) trait ArchetypeTable {
    type A: Archetype;

    fn new() -> Self;

    fn insert(&mut self, data: Self::A, entity_handle: EntityHandle) -> InstanceHandle;

    fn remove(&mut self, handle: InstanceHandle);

    fn collect<'a>(&'a self, collector: &mut RenderFrame<'a>);

    fn write_draw_data(&mut self, handle: &InstanceHandle, draws: Vec<DrawData>);
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
    pub(super) draw_palette: DrawPalette,
}
#[cfg(test)]
impl APositionTable {
    pub fn get_positions(&self) -> Vec<GlobalTransform> {
        self.positions.clone()
    }
}

impl ArchetypeTable for APositionTable {
    type A = APosition;

    fn write_draw_data(&mut self, handle: &InstanceHandle, draws: Vec<DrawData>) {
        let offset = self.draw_palette.data.len();
        let dense_idx = self
            .arena
            .resolve(handle)
            .expect("cannot resolve the dense idx of this instance");
        self.draw_palette.chunks[dense_idx].populate_data(offset, draws.len());
        self.draw_palette.data.extend(draws);
    }

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
            draw_palette: DrawPalette::new(),
        }
    }

    fn insert(&mut self, data: APosition, entity_handle: EntityHandle) -> InstanceHandle {
        self.positions.push(data.position);
        self.draw_palette.chunks.push(PaletteChunk::default());
        self.arena.insert(entity_handle)
    }

    fn remove(&mut self, handle: InstanceHandle) {
        // TODO: draw data becomes orphaned once the instance is gone, so we need a defrag algorithm
        // to reduce memory leaks here. Not as much of a concern until im running at scale
        if let Some(idx_of_goner) = self.arena.remove(handle) {
            let last = self.positions.len() - 1;
            self.positions.swap(idx_of_goner, last);
            self.draw_palette.chunks.swap(idx_of_goner, last);
        } else {
            self.positions.pop();
        }
    }
}
