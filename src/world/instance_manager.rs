use std::collections::HashMap;

use crate::{
    app::renderer::{DrawItem, DrawPacket, InstanceUploadJob},
    util::types::{GlobalTransform, LocalTransform},
    world::{
        InstanceUploadQuery,
        entity_manager::{EntityHandle, EntityManager, RenderData, Renderables},
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

#[derive(Default)]
struct OffsetMap {
    a_postion_offset: u16,
    // other tables
}
impl OffsetMap {
    fn offset_of(&self, a_id: ArchetypeId) -> u16 {
        match a_id {
            ArchetypeId::Position => self.a_postion_offset,
        }
    }
}

struct InstanceDataCollector<'a> {
    offset_map: OffsetMap,
    global_transforms: Vec<&'a [GlobalTransform]>,
    gt_len: usize,
}
impl<'a> InstanceDataCollector<'a> {
    fn new() -> Self {
        Self {
            gt_len: 0,
            offset_map: OffsetMap::default(),
            global_transforms: Vec::new(),
        }
    }
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

    #[cfg(test)]
    pub fn get_groups(&self) -> &Vec<RenderGroup> {
        &self.render_groups
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
        entity_handle: &EntityHandle,
        data: Box<dyn Archetype>,
    ) -> InstanceHandle {
        data.insert_self(self, entity_handle)
    }

    pub fn do_stuff(
        &mut self,
        instance_handle: &InstanceHandle,
        entity_manager: &EntityManager,
    ) -> InstanceUploadData {
        let is_instanced = self
            .entity_group_index
            .contains_key(&instance_handle.entity_handle);
        let mut renderables =
            entity_manager.get_entity_renderables(&instance_handle.entity_handle, is_instanced);

        if is_instanced {
            let group_id = self
                .entity_group_index
                .get(&instance_handle.entity_handle)
                .unwrap();
            let group = self
                .render_groups
                .get_mut(*group_id)
                .expect("group should exist, maybe you deleted from the value, but not the entry?");
            group.instance_handles.push(instance_handle.clone());
        } else {
            let mut views = Vec::<RenderView>::with_capacity(
                renderables
                    .common
                    .as_ref()
                    .expect("this is the first instance, so it should have common data")
                    .len(),
            );

            for render_data in renderables.common.take().unwrap() {
                match render_data {
                    RenderData::MeshRenderable {
                        gpu_alloc_handle,
                        pnu_vertex_ranges,
                        pnujw_vertex_ranges,
                        index_ranges,
                    } => {
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
                    }
                }
            }
        }

        renderables.instance_data
    }

    // pub fn update_render_state<'a>(
    //     &mut self,
    //     instance_handle: &InstanceHandle,
    //     renderables: Renderables<'a>,
    // ) {
    //     if let Some(group_id) = self.entity_group_index.get(&instance_handle.entity_handle) {
    //         let group = self
    //             .render_groups
    //             .get_mut(*group_id)
    //             .expect("group should exist, maybe you deleted from the value, but not the entry?");
    //         group.instance_handles.push(instance_handle.clone());
    //     } else {
    //         let mut views: Vec<RenderView> = Vec::with_capacity(renderables.instance_data.len());
    //         for instance_data in renderables.instance_data {
    //             match instance_data {
    //                 InstanceRenderData::MeshRenderable {
    //                     gpu_alloc_handle,
    //                     pnu_vertex_ranges,
    //                     pnujw_vertex_ranges,
    //                     index_ranges,
    //                 } => {
    //                     let view = RenderView {
    //                         gpu_handle: gpu_alloc_handle,

    //                         pnu_draws: pnu_vertex_ranges.map(|pnu| DrawSet {
    //                             primtitive_ranges: pnu,
    //                             index_ranges: index_ranges.clone(),
    //                         }),
    //                         pnujw_draws: pnujw_vertex_ranges.map(|pnujw| DrawSet {
    //                             primtitive_ranges: pnujw,
    //                             index_ranges: index_ranges,
    //                         }),
    //                     };
    //                     views.push(view);
    //                 }
    //             }
    //         }

    //         self.render_groups.push(RenderGroup {
    //             instance_handles: vec![instance_handle.clone()],
    //             views,
    //         });

    //         self.entity_group_index.insert(
    //             instance_handle.entity_handle.clone(),
    //             self.render_groups.len() - 1,
    //         );
    //     }
    // }

    pub fn despawn(&mut self, handle: InstanceHandle) {
        match handle.archetype {
            ArchetypeId::Position => self.pos.remove(handle),
        }
        // TODO: other tables
    }

    pub fn gen_draw_calls<'frame>(&'frame self, packet: &mut DrawPacket) {
        let mut collector = InstanceDataCollector::new();
        self.pos.collect(&mut collector, 0);

        for group in self.render_groups.iter() {
            for view in group.views.iter() {
                if let Some(pnu) = &view.pnu_draws {
                    let entry = packet
                        .pnu
                        .entry(view.gpu_handle.clone())
                        .or_insert_with(Vec::new);
                    for instance_handle in group.instance_handles.iter() {
                        // calculate the instance idx of each draw call
                        let offset = collector.offset_map.offset_of(instance_handle.archetype);
                        let instance_idx =
                            self.resolve_idx(instance_handle).expect("should be valid") as u32
                                + offset as u32;
                        if let Some(bindings) = self.gpu_bindings.get(instance_handle) {
                            for (i, pr) in pnu.primtitive_ranges.iter().enumerate() {
                                entry.push(DrawItem::new(
                                    bindings.lt_offset,
                                    instance_idx..instance_idx + 1,
                                    pr.clone(),
                                    pnu.index_ranges.as_ref().map(|x| x[i].clone()),
                                ))
                            }
                        }
                    }
                }
                if let Some(pnujw) = &view.pnujw_draws {
                    let entry = packet
                        .pnujw
                        .entry(view.gpu_handle.clone())
                        .or_insert_with(Vec::new);
                    for instance_handle in group.instance_handles.iter() {
                        // calculate the instance idx of each draw call
                        let offset = collector.offset_map.offset_of(instance_handle.archetype);
                        let instance_idx =
                            self.resolve_idx(instance_handle).expect("should be valid") as u32
                                + offset as u32;
                        if let Some(bindings) = self.gpu_bindings.get(instance_handle) {
                            for (i, pr) in pnujw.primtitive_ranges.iter().enumerate() {
                                entry.push(DrawItem::new(
                                    bindings.lt_offset,
                                    instance_idx..instance_idx + 1,
                                    pr.clone(),
                                    pnujw.index_ranges.as_ref().map(|x| x[i].clone()),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}
