use std::collections::HashMap;

use crate::{
    app::renderer::{InstanceUploadJob, renderer::InstanceDataCollector},
    util::types::{GlobalTransform, LocalTransform},
    world::{
        entity_manager::{EntityHandle, InstanceRenderData, Renderables},
        index_arena::InstanceArenaNew,
        world::{DrawSet, InstanceUploadData, RenderGroup, RenderView},
    },
};

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
        entity_handle: &EntityHandle,
    ) -> InstanceHandle {
        manager.pos.insert(*self, *entity_handle)
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

#[derive(Debug)]
pub struct InstanceGPUBindings {
    pub lt_offset: u32,
}

pub struct InstanceManager {
    pub(super) next_id: u16,
    gpu_bindings: HashMap<InstanceHandle, InstanceGPUBindings>,
    pub pos: APositionTable,
    render_groups: Vec<RenderGroup>,
    pub(super) entity_group_index: HashMap<EntityHandle, usize>,
}

impl InstanceManager {
    #[cfg(test)]
    pub fn get_all_instances(&self) -> Vec<InstanceHandle> {
        self.gpu_bindings.keys().cloned().collect()
    }

    #[cfg(test)]
    pub fn get_pos_table(&self) -> &APositionTable {
        &self.pos
    }

    pub fn update_gpu_bindings(&mut self, data: (InstanceHandle, InstanceGPUBindings)) {
        self.gpu_bindings.insert(data.0, data.1);
    }
    pub(super) fn new() -> Self {
        Self {
            next_id: 0,
            pos: APositionTable::new(),
            gpu_bindings: HashMap::new(),
            render_groups: Vec::new(),
            entity_group_index: HashMap::new(),
        }
    }

    pub fn resolve_idx(&self, handle: &InstanceHandle) -> Option<usize> {
        match handle.archetype {
            ArchetypeId::Position => self.pos.arena.resolve(handle),
        }
    }

    pub(super) fn spawn<'a>(
        &mut self,
        renderables: Renderables<'a>,
        data: Box<dyn Archetype>,
    ) -> InstanceUploadData {
        let instance_handle = data.insert_self(self, renderables.entity_handle);
        let mut res = InstanceUploadData {
            instance_handle,
            local_transforms: None,
        };

        let mut views: Vec<RenderView> = Vec::with_capacity(renderables.instance_data.len());
        for instance_data in renderables.instance_data {
            match instance_data {
                InstanceRenderData::MeshRenderable {
                    gpu_alloc_handle,
                    pnu_vertex_ranges,
                    pnujw_vertex_ranges,
                    index_ranges,
                    local_transforms,
                } => {
                    // BYTECODE TO UPLOAD LOCAL TRANSFORMS
                    let view = RenderView {
                        gpu_handle: gpu_alloc_handle,

                        pnu_draws: pnu_vertex_ranges.map(|pnu| DrawSet {
                            primtitive_ranges: pnu,
                            index_ranges: index_ranges.clone(),
                        }),
                        pnujw_draws: pnujw_vertex_ranges.map(|pnujw| DrawSet {
                            primtitive_ranges: pnujw,
                            index_ranges: index_ranges,
                        }),
                    };
                    views.push(view);

                    res.local_transforms = Some(local_transforms.lt);
                }
                _ => todo!("other instance data types"),
            }
        }
        res
    }

    pub fn despawn(&mut self, handle: InstanceHandle) {
        match handle.archetype {
            ArchetypeId::Position => self.pos.remove(handle),
        }
        // TODO: other tables
    }
}
