use std::{collections::HashMap, sync::Arc};

use cgmath::SquareMatrix;
use time::{Duration, ext::InstantExt};

use crate::{
    animation::animation::{Animation, AnimationSample, EntityAnimations},
    app::{
        app::AppCommand,
        renderer::{DrawItem, DrawPacket},
    },
    util::types::{GlobalTransform, LocalTransform, Mat4F32},
    world::{
        RenderKey, WorldUpdateError,
        entity_manager::{EntityHandle, EntityManager, RenderData},
        index_arena::InstanceArenaNew,
        world::{
            DrawSet, InstanceUploadData, LocalTransformData, LocalTransformsNew, RenderGroup,
            RenderView,
        },
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
    PositionAnimated = 1,
}
impl TryFrom<u16> for ArchetypeId {
    type Error = ();
    fn try_from(v: u16) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Position),
            _ => Err(()),
        }
    }
}

trait ArchetypeTable {
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

pub struct APositionAnimated {
    pub position: GlobalTransform,
    pub time_delta: f32,
}

impl ArchetypeIdent for APositionAnimated {
    const ARCHETYPE_ID: ArchetypeId = ArchetypeId::PositionAnimated;
}

impl Archetype for APositionAnimated {
    fn insert_self(
        self: Box<Self>,
        manager: &mut InstanceManager,
        entity_handle: &EntityHandle,
    ) -> InstanceHandle {
        todo!()
    }
}

pub struct APositionAnimatedTable {
    pub(super) positions: Vec<GlobalTransform>,
    pub(super) time_deltas: Vec<f32>,
    pub(super) arena: InstanceArenaNew<APositionAnimated>,
}

impl ArchetypeTable for APositionAnimatedTable {
    type A = APositionAnimated;

    fn collect<'a>(&'a self, collector: &mut RenderFrame<'a>) {
        if !self.positions.is_empty() {
            collector
                .global_transforms
                .push(bytemuck::cast_slice(&self.positions));
            todo!("deltas?")
        }
        todo!()
    }

    fn new() -> Self {
        Self {
            positions: Vec::new(),
            time_deltas: Vec::new(),
            arena: InstanceArenaNew::new(),
        }
    }
    fn remove(&mut self, handle: InstanceHandle) {
        let last = self.positions.len() - 1;
        if let Some(idx_of_goner) = self.arena.remove(handle) {
            self.positions.swap(idx_of_goner, last);
            self.time_deltas.swap(idx_of_goner, last);
        } else {
            self.positions.pop();
            self.time_deltas.pop();
        }
    }

    fn insert(&mut self, data: Self::A, entity_handle: EntityHandle) -> InstanceHandle {
        self.positions.push(data.position);
        self.time_deltas.push(data.time_delta);
        self.arena.insert(entity_handle)
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
            arena: InstanceArenaNew::new(),
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

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct InstanceHandle {
    pub archetype: ArchetypeId,
    pub entity_handle: EntityHandle,
    pub instance_id: u16,
    pub generation: u16,
}

impl RenderKey for InstanceHandle {
    fn as_key(&self) -> u64 {
        let i = self.instance_id as u64;
        let e = (self.entity_handle.0 as u64) << 16;
        let a = (self.archetype as u64) << 32;
        let g = (self.generation as u64) << 48;
        i | e | a | g
    }

    fn from_key(key: u64) -> Self {
        let instance = (key & 0xFFFF) as u16;
        let entity = ((key >> 16) & 0xFFFF) as u16;
        let archetype = ((key >> 32) & 0xFFFF) as u16;
        let generation = ((key >> 48) & 0xFFFF) as u16;

        Self {
            archetype: ArchetypeId::try_from(archetype).expect("invalid archetype in key"),
            entity_handle: EntityHandle(entity),
            generation,
            instance_id: instance,
        }
    }
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

pub struct AnimationInstance {
    pub samples: Vec<AnimationSample>,
    animation_idx: usize,
    pub start_time: std::time::Instant,
    pub buffer: Vec<Mat4F32>,
    instance_handle: InstanceHandle,
}

#[cfg(test)]
impl AnimationInstance {
    pub fn new_for_test(samples: Vec<AnimationSample>, count: usize) -> Self {
        use std::time::Instant;

        Self {
            samples,
            animation_idx: 0,
            start_time: Instant::now(),
            buffer: vec![[[0f32; 4]; 4]; count],
            instance_handle: InstanceHandle::mock(ArchetypeId::Position, EntityHandle(0), 0, 0),
        }
    }
}

#[derive(Default)]
pub struct AnimationController {
    registered_animations: HashMap<EntityHandle, EntityAnimations>,
    active_animations: Vec<AnimationInstance>,
}

impl AnimationController {
    /// time offset is unsafe: only use if you are sure the offset is a valid value for the animation, or if
    /// the animation is repeating
    fn activate_animations(
        &mut self,
        instance_handle: &InstanceHandle,
        anim_idx: usize,
        time_offset: Option<f32>,
    ) -> Option<()> {
        let entity_animation = self
            .registered_animations
            .get(&instance_handle.entity_handle)?;

        let buffer: Vec<Mat4F32> = entity_animation
            .local_transforms
            .iter()
            .map(|lt| **lt)
            .collect();
        self.active_animations.push(AnimationInstance {
            samples: entity_animation.animation[anim_idx].init_samples(),
            buffer,
            animation_idx: anim_idx,
            start_time: std::time::Instant::now()
                .add_signed(Duration::milliseconds(time_offset.unwrap_or(0.0) as i64)),
            instance_handle: instance_handle.clone(),
        });
        Some(())
    }
}
pub struct InstanceManager {
    pub(super) next_id: u16,
    gpu_bindings: HashMap<InstanceHandle, InstanceGPUBindings>,
    pub pos: APositionTable,
    pub posAnim: APositionAnimatedTable,
    render_groups: Vec<RenderGroup>,
    pub(super) entity_group_index: HashMap<EntityHandle, usize>,
    animation_controller: AnimationController,
}

impl InstanceManager {
    #[cfg(test)]
    pub fn assert_local_transforms_exist(&self, instance_handle: &InstanceHandle) {
        assert!(
            self.animation_controller
                .registered_animations
                .contains_key(&instance_handle.entity_handle)
        )
    }

    #[cfg(test)]
    pub fn get_active_animations(&self) -> &[AnimationInstance] {
        &self.animation_controller.active_animations
    }

    #[cfg(test)]
    pub fn get_animation_ref(
        &self,
        entity_handle: &EntityHandle,
        index: usize,
    ) -> &Arc<dyn Animation> {
        &self
            .animation_controller
            .registered_animations
            .get(entity_handle)
            .unwrap()
            .animation[index]
    }

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
            posAnim: APositionAnimatedTable::new(),
            gpu_bindings: HashMap::new(),
            render_groups: Vec::new(),
            entity_group_index: HashMap::new(),
            animation_controller: AnimationController::default(),
        }
    }

    pub fn activate_animation(
        &mut self,
        instance_handle: &InstanceHandle,
        anim_idx: usize,
        offset: Option<f32>,
    ) {
        self.animation_controller
            .activate_animations(instance_handle, anim_idx, offset);
    }
    pub fn resolve_idx(&self, handle: &InstanceHandle) -> Option<usize> {
        match handle.archetype {
            ArchetypeId::Position => self.pos.arena.resolve(handle),
            ArchetypeId::PositionAnimated => self.posAnim.arena.resolve(handle),
        }
    }

    pub(super) fn update(&mut self, commands: &mut Vec<AppCommand>) {
        if let Some(command) = commands.pop() {
            let mut handle = None;
            match command {
                AppCommand::DoSomething => {
                    for (found, _) in self.gpu_bindings.iter() {
                        handle = Some(found.clone());
                    }
                }
            }
            if handle.is_none() {
                commands.push(command);
            } else {
                self.activate_animation(handle.as_ref().unwrap(), 0, None);
            }
        }
        // DO ANIMATIONS
        for active_animation in self.animation_controller.active_animations.iter_mut() {
            let animation = &self
                .animation_controller
                .registered_animations
                .get(&active_animation.instance_handle.entity_handle)
                .unwrap()
                .animation[active_animation.animation_idx];

            let now = std::time::Instant::now();
            let time_delta: f32 = (now - active_animation.start_time).as_secs_f32();
            animation.get_animation_frame(
                time_delta,
                active_animation,
                &cgmath::Matrix4::<f32>::identity(),
            );
        }
    }

    pub(super) fn spawn(
        &mut self,
        entity_handle: &EntityHandle,
        entity_manager: &EntityManager,
        data: Box<dyn Archetype>,
    ) -> Result<InstanceUploadData, WorldUpdateError> {
        let instance_handle = &data.insert_self(self, entity_handle);
        let mut res = InstanceUploadData {
            instance_handle: instance_handle.clone(),
            local_transforms: LocalTransformsNew::Uninit,
        };
        let is_instanced = self
            .entity_group_index
            .contains_key(&instance_handle.entity_handle);

        if is_instanced {
            return Ok(entity_manager.get_entity_cloned(&instance_handle));
        } else {
            let mut renderables = entity_manager
                .get_entity_render_data(&instance_handle)
                .expect("renderables fetch fail");

            // ******* MESH DATA ********
            let mut views = Vec::<RenderView>::with_capacity(renderables.mesh_renderables.len());

            for (alloc_handle, mesh_data) in renderables.mesh_renderables.drain(..) {
                let view = RenderView {
                    gpu_handle: alloc_handle,
                    pnu_draws: mesh_data.pnu_vertex_ranges.map(|pnu| DrawSet {
                        mesh_map: mesh_data.pnu_mesh_map,
                        primtitive_ranges: pnu,
                        index_ranges: mesh_data.index_ranges.clone(),
                    }),
                    pnujw_draws: mesh_data.pnujw_vertex_ranges.map(|pnujw| DrawSet {
                        mesh_map: mesh_data.pnujw_mesh_map,
                        primtitive_ranges: pnujw,
                        index_ranges: mesh_data.index_ranges.clone(),
                    }),
                };

                views.push(view);
                match &mut res.local_transforms {
                    LocalTransformsNew::Uninit => {
                        res.local_transforms = LocalTransformsNew::Owned {
                            data: mesh_data.local_transforms,
                        }
                    }
                    LocalTransformsNew::Owned { data } => data.extend(mesh_data.local_transforms),
                    _ => panic!("unexpected local transform data val"),
                }
            }

            // ******** ANIMATION DATA *********
            if let Some(entity_animations) = renderables.animations {
                self.animation_controller
                    .registered_animations
                    .insert(instance_handle.entity_handle.clone(), entity_animations);
            }
        }

        Ok(res)
    }
    // pub(super) fn spawn(
    //     &mut self,
    //     entity_handle: &EntityHandle,
    //     entity_manager: &EntityManager,
    //     data: Box<dyn Archetype>,
    // ) -> Result<InstanceUploadData, WorldUpdateError> {
    //     let instance_handle = &data.insert_self(self, entity_handle);
    //     let is_instanced = self
    //         .entity_group_index
    //         .contains_key(&instance_handle.entity_handle);

    //     let mut renderables = entity_manager
    //         .get_entity_renderables(instance_handle, is_instanced)
    //         .map_err(|_| {
    //             WorldUpdateError::RenderablesNotAvailable(instance_handle.entity_handle)
    //         })?;

    //     if is_instanced {
    //         let group_id = self
    //             .entity_group_index
    //             .get(&instance_handle.entity_handle)
    //             .unwrap();
    //         let group = self
    //             .render_groups
    //             .get_mut(*group_id)
    //             .expect("group should exist, maybe you deleted from the value, but not the entry?");
    //         group.instance_handles.push(instance_handle.clone());

    //         if matches!(
    //             renderables.instance_data.as_ref().unwrap().local_transforms,
    //             LocalTransformData::NeedsDonor
    //         ) {
    //             renderables.instance_data.as_mut().unwrap().local_transforms =
    //                 LocalTransformData::FromShared {
    //                     donor: group.instance_handles[0].clone(),
    //                 }
    //         }
    //     } else {
    //         let mut views = Vec::<RenderView>::with_capacity(
    //             renderables
    //                 .common
    //                 .as_ref()
    //                 .expect("this is the first instance, so it should have common data")
    //                 .len(),
    //         );

    //         for render_data in renderables.common.take().unwrap() {
    //             match render_data {
    //                 RenderData::MeshRenderable {
    //                     gpu_alloc_handle,
    //                     pnu_vertex_ranges,
    //                     pnu_mesh_map,
    //                     pnujw_mesh_map,
    //                     pnujw_vertex_ranges,
    //                     index_ranges,
    //                 } => {
    //                     let view = RenderView {
    //                         gpu_handle: gpu_alloc_handle,
    //                         pnu_draws: pnu_vertex_ranges.map(|pnu| DrawSet {
    //                             mesh_map: pnu_mesh_map,
    //                             primtitive_ranges: pnu,
    //                             index_ranges: index_ranges.clone(),
    //                         }),
    //                         pnujw_draws: pnujw_vertex_ranges.map(|pnujw| DrawSet {
    //                             mesh_map: pnujw_mesh_map,
    //                             primtitive_ranges: pnujw,
    //                             index_ranges: index_ranges,
    //                         }),
    //                     };
    //                     views.push(view);
    //                 }
    //                 RenderData::AnimationData { animations } => {
    //                     self.animation_controller.insert_animation_refs(
    //                         animations,
    //                         instance_handle.clone(),
    //                         &renderables.instance_data.as_ref().unwrap().local_transforms,
    //                     );
    //                 }
    //             }
    //         }
    //         // ADD GROUP
    //         self.render_groups.push(RenderGroup {
    //             instance_handles: vec![instance_handle.clone()],
    //             views,
    //         });
    //         self.entity_group_index.insert(
    //             instance_handle.entity_handle.clone(),
    //             self.render_groups.len() - 1,
    //         );
    //     }

    //     Ok(renderables.instance_data.unwrap())
    // }

    pub fn despawn(&mut self, handle: InstanceHandle) {
        match handle.archetype {
            ArchetypeId::Position => self.pos.remove(handle),
            ArchetypeId::PositionAnimated => self.posAnim.remove(handle),
        }
        // TODO: other tables
    }

    // Calulate the offset based on the length of the archetype tables, and a defined order in which
    // the tables are read
    pub fn offset_of(&self, archetype: ArchetypeId) -> usize {
        match archetype {
            ArchetypeId::Position => 0,
            ArchetypeId::PositionAnimated => self.pos.positions.len() - 1,
        }
    }

    pub fn gen_draw_calls<'frame>(&'frame self, packet: &mut DrawPacket) {
        for group in self.render_groups.iter() {
            for view in group.views.iter() {
                if let Some(pnu) = &view.pnu_draws {
                    let entry = packet
                        .pnu
                        .entry(view.gpu_handle.clone())
                        .or_insert_with(Vec::new);
                    for instance_handle in group.instance_handles.iter() {
                        // calculate the instance idx of each draw call
                        let offset = self.offset_of(instance_handle.archetype);
                        let instance_idx =
                            self.resolve_idx(instance_handle).expect("should be valid") as u32
                                + offset as u32;
                        if let Some(bindings) = self.gpu_bindings.get(instance_handle) {
                            for (i, pr) in pnu.primtitive_ranges.iter().enumerate() {
                                entry.push(DrawItem::new(
                                    bindings.lt_offset + pnu.mesh_map[i],
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
                        let offset = self.offset_of(instance_handle.archetype);
                        let instance_idx =
                            self.resolve_idx(instance_handle).expect("should be valid") as u32
                                + offset as u32;
                        if let Some(bindings) = self.gpu_bindings.get(instance_handle) {
                            for (i, pr) in pnujw.primtitive_ranges.iter().enumerate() {
                                entry.push(DrawItem::new(
                                    bindings.lt_offset + pnujw.mesh_map[i],
                                    instance_idx..instance_idx + 1,
                                    pr.clone(),
                                    pnujw.index_ranges.as_ref().map(|x| x[i].clone()),
                                ));
                            }
                        } else {
                            // skip rendering
                        }
                    }
                }
            }
        }
    }

    pub fn prepare_render_frame<'frame>(&'frame self) -> RenderFrame<'frame> {
        let mut render_frame = RenderFrame::default();
        self.pos.collect(&mut render_frame);

        for animation_instance in self.animation_controller.active_animations.iter() {
            let lt_offset = self
                .gpu_bindings
                .get(&animation_instance.instance_handle)
                .unwrap()
                .lt_offset;

            render_frame.rigid_animation_data.push(AnimationUpdate {
                buffer_offset: lt_offset,
                transforms: bytemuck::cast_slice(&animation_instance.buffer),
            });
        }
        render_frame
    }
}

#[derive(Debug)]
pub struct AnimationUpdate<'frame> {
    pub buffer_offset: u32,
    pub transforms: &'frame [u8],
}

#[derive(Debug, Default)]
pub struct RenderFrame<'frame> {
    pub global_transforms: Vec<&'frame [u8]>,
    pub local_transforms: Vec<&'frame [u8]>,
    pub rigid_animation_data: Vec<AnimationUpdate<'frame>>,
}
