use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;

use crate::{
    animation::animation::EntityAnimations,
    app::{
        app::AppCommand,
        renderer::{DrawItem, DrawPacket},
    },
    world::{
        WorldUpdateError,
        entity_manager::{EntityHandle, entity_manager::EntityManager},
        instance_manager::{
            Archetype, ArchetypeId, InstanceGPUBindings, InstanceHandle, RenderFrame,
            animation_controller::AnimationController,
            archetype_table::{APositionTable, ArchetypeTable},
        },
        world::{
            DrawSet, InstanceUploadData, InverseBindMatrices, JointTransforms, LocalTransforms,
            RenderGroup, RenderView,
        },
    },
};
#[cfg(test)]
use crate::{
    animation::animation::{Animation, AnimationInstance},
    util::types::GlobalTransform,
};
pub struct InstanceManager {
    pub(super) _next_id: u16,
    gpu_bindings: HashMap<InstanceHandle, InstanceGPUBindings>,
    pub(super) pos: APositionTable,
    render_groups: Vec<RenderGroup>,
    pub(super) entity_group_index: HashMap<EntityHandle, usize>,
    animation_controller: AnimationController,
}

impl InstanceManager {
    #[cfg(test)]
    pub fn assert_animation_exists(&self, instance_handle: &InstanceHandle) {
        assert!(
            self.animation_controller
                .registered_animations
                .contains_key(&instance_handle.entity_handle)
        )
    }
    #[cfg(test)]
    pub fn get_gpu_bindings(&self, handle: &InstanceHandle) -> Option<&InstanceGPUBindings> {
        self.gpu_bindings.get(handle)
    }

    #[cfg(test)]
    pub fn get_joint_slot_map(&self, entity_handle: &EntityHandle) -> &Vec<usize> {
        &self
            .animation_controller
            .registered_animations
            .get(entity_handle)
            .expect("entity must be registered")
            .skin_offset_map
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
    pub fn get_entity_animation(&self, entity_handle: &EntityHandle) -> Option<&EntityAnimations> {
        self.animation_controller
            .registered_animations
            .get(entity_handle)
    }

    #[cfg(test)]
    pub fn get_all_instances(&self) -> Vec<InstanceHandle> {
        self.gpu_bindings.keys().cloned().collect()
    }

    #[cfg(test)]
    pub fn get_pos_table_positions(&self) -> Vec<GlobalTransform> {
        self.pos.get_positions()
    }

    #[cfg(test)]
    pub fn get_groups(&self) -> &Vec<RenderGroup> {
        &self.render_groups
    }

    pub fn update_gpu_bindings(&mut self, data: (InstanceHandle, InstanceGPUBindings)) {
        self.gpu_bindings.insert(data.0, data.1);
    }
    pub fn new() -> Self {
        Self {
            _next_id: 0,
            pos: APositionTable::new(),
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
        }
    }

    pub fn update(&mut self, commands: &mut Vec<AppCommand>) {
        if let Some(command) = commands.pop() {
            let idx;
            let mut handle = None;
            match command {
                AppCommand::One => idx = 0,
                AppCommand::Two => idx = 1,
                AppCommand::Three => idx = 2,
            }
            if self.gpu_bindings.is_empty() {
                commands.push(command);
            } else {
                for stored_handle in self.gpu_bindings.keys() {
                    handle = Some(stored_handle.clone());
                }
                self.activate_animation(handle.as_ref().unwrap(), idx, None);
            }
        }
        self.animation_controller.update();
    }

    #[cfg(test)]
    pub fn run_animations(&mut self, time_delta: f32) {
        self.animation_controller.run_animations(time_delta);
    }

    #[cfg(test)]
    pub fn get_buffer_slot_map(&self, instance_idx: usize) -> &Vec<usize> {
        let a = &self.animation_controller.active_animations[instance_idx];
        let entity_anim = self
            .animation_controller
            .registered_animations
            .get(&a.instance_handle.entity_handle)
            .unwrap();

        &entity_anim.mesh_slot_map
    }

    pub fn spawn(
        &mut self,
        entity_handle: &EntityHandle,
        entity_manager: &EntityManager,
        data: Box<dyn Archetype>,
    ) -> Result<InstanceUploadData, WorldUpdateError> {
        let instance_handle = &data.insert_self(self, entity_handle);
        let is_instanced = self
            .entity_group_index
            .contains_key(&instance_handle.entity_handle);

        if is_instanced {
            let group_idx = self.entity_group_index.get(entity_handle).unwrap();
            let group = self.render_groups.get_mut(*group_idx).unwrap();
            group.instance_handles.push(instance_handle.clone());
            let mut instance_upload_data = entity_manager.get_entity_cloned(&instance_handle);
            match instance_upload_data.local_transforms {
                LocalTransforms::NeedsCopy => {
                    instance_upload_data.local_transforms = LocalTransforms::CopiedFrom {
                        donor: group.instance_handles[0].clone(), // TODO: manage shared slots
                    }
                }
                LocalTransforms::NeedsShared => {
                    instance_upload_data.local_transforms = LocalTransforms::SharedWith {
                        donor: group.instance_handles[0].clone(), // TODO: manage shared slots
                    }
                }
                _ => panic!("unexpected local transform value"),
            }
            match instance_upload_data.joint_transforms {
                JointTransforms::NeedsShared => {
                    instance_upload_data.joint_transforms = JointTransforms::SharedWith {
                        donor: group.instance_handles[0].clone(),
                    }
                }
                JointTransforms::NeedsCopy => {
                    instance_upload_data.joint_transforms = JointTransforms::CopiedFrom {
                        donor: group.instance_handles[0].clone(),
                    }
                }
                JointTransforms::None => {
                    //
                }
                _ => panic!("unexpected JointTransforms value"),
            }
            return Ok(instance_upload_data);
        } else {
            let mut res = InstanceUploadData {
                instance_handle: instance_handle.clone(),
                local_transforms: LocalTransforms::Uninit,
                joint_transforms: JointTransforms::None,
                ibms: InverseBindMatrices::Uninit,
            };
            let mut renderables = entity_manager
                .get_entity_render_data(&instance_handle)
                .expect("renderables fetch fail");

            // ******* MESH DATA ********
            let mut views = Vec::<RenderView>::with_capacity(renderables.mesh_renderables.len());

            for (alloc_handle, mesh_data) in renderables.mesh_renderables.drain(..) {
                let view = RenderView {
                    gpu_handle: alloc_handle,
                    pnu_draws: mesh_data.pnu_vertex_ranges.map(|pnu| DrawSet {
                        joint_map: vec![], // TODO: seprate draw set struct for pnu to avoid this?
                        mesh_map: mesh_data.pnu_mesh_map,
                        primtitive_ranges: pnu,
                        index_ranges: mesh_data.index_ranges.clone(),
                    }),
                    pnujw_draws: mesh_data.pnujw_vertex_ranges.map(|pnujw| DrawSet {
                        joint_map: mesh_data.joint_map,
                        mesh_map: mesh_data.pnujw_mesh_map,
                        primtitive_ranges: pnujw,
                        index_ranges: mesh_data.index_ranges.clone(),
                    }),
                };

                views.push(view);
                match &mut res.local_transforms {
                    LocalTransforms::Uninit => {
                        res.local_transforms = LocalTransforms::Owned {
                            data: mesh_data.local_transforms,
                        }
                    }
                    LocalTransforms::Owned { data } => data.extend(mesh_data.local_transforms),
                    _ => panic!("unexpected local transform data val"),
                }
            }

            self.entity_group_index
                .insert(*entity_handle, self.render_groups.len());
            self.render_groups.push(RenderGroup {
                instance_handles: vec![instance_handle.clone()],
                views,
            });

            // ******** ANIMATION DATA *********
            if let Some(entity_animation_data) = renderables.animations {
                res.joint_transforms = JointTransforms::Uninit;
                if !entity_animation_data.joint_transforms.is_empty() {
                    match &mut res.joint_transforms {
                        JointTransforms::Uninit | JointTransforms::None => {
                            res.joint_transforms = JointTransforms::Owned {
                                data: entity_animation_data.joint_transforms.clone(),
                            }
                        }
                        JointTransforms::Owned { data } => {
                            data.extend(entity_animation_data.joint_transforms.clone());
                        }
                        _ => panic!("unexpected joint transform data"),
                    }
                    match res.ibms {
                        InverseBindMatrices::None | InverseBindMatrices::Uninit => {
                            res.ibms = InverseBindMatrices::Owned {
                                data: entity_animation_data.inverse_bind_matrices,
                            }
                        }
                        InverseBindMatrices::Owned { mut data } => {
                            data.extend(entity_animation_data.inverse_bind_matrices);
                            res.ibms = InverseBindMatrices::Owned { data: data }
                        }
                        _ => panic!("unexpected ibm result"),
                    }
                } else {
                    res.joint_transforms = JointTransforms::None;
                }
                self.animation_controller.registered_animations.insert(
                    instance_handle.entity_handle.clone(),
                    EntityAnimations {
                        animation: entity_animation_data.animation,
                        local_transforms: entity_animation_data.local_transforms,
                        joint_transforms: entity_animation_data.joint_transforms,
                        mesh_slot_map: entity_animation_data.mesh_slot_map,
                        skin_offset_map: entity_animation_data.skin_offset_map,
                    },
                );
            } else {
                res.ibms = InverseBindMatrices::None;
            }
            Ok(res)
        }
    }

    pub fn despawn(&mut self, handle: InstanceHandle) {
        match handle.archetype {
            ArchetypeId::Position => self.pos.remove(handle),
        }
        // TODO: other tables
    }

    // Calulate the offset based on the length of the archetype tables, and a defined order in which
    // the tables are read
    pub fn offset_of(&self, archetype: ArchetypeId) -> usize {
        match archetype {
            ArchetypeId::Position => 0,
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
                                entry.push(DrawItem {
                                    joint_offset: None,
                                    lt_idx: bindings.lt_offset + pnu.mesh_map[i],
                                    instances: instance_idx..instance_idx + 1,
                                    primitives: pr.clone(),
                                    indices: pnu.index_ranges.as_ref().map(|x| x[i].clone()),
                                });
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
                                entry.push(DrawItem {
                                    joint_offset: bindings
                                        .joint_offset
                                        .map(|offset| offset + pnujw.joint_map[i]),
                                    lt_idx: bindings.lt_offset + pnujw.mesh_map[i],
                                    instances: instance_idx..instance_idx + 1,
                                    primitives: pr.clone(),
                                    indices: pnujw.index_ranges.as_ref().map(|x| x[i].clone()),
                                });
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

        self.animation_controller
            .prepare_animation_frame(&mut render_frame, &self.gpu_bindings);
        render_frame
    }
}
