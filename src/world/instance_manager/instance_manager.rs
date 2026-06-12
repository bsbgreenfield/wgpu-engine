use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;

use crate::{
    animation::animation::EntityAnimations,
    app::{
        app::AppCommand,
        renderer::{DrawPacket, PrototypeHandle, renderer::GPUInstanceHandle},
    },
    asset_manager::asset_manager_new::AssetManager,
    world::{
        WorldUpdateError,
        entity_manager::{
            EntityHandle,
            entity_manager::{EntityManager, Renderables},
        },
        instance_manager::{
            Archetype, ArchetypeId, InstanceGPUBindings, InstanceHandle, RenderFrame,
            animation_controller::AnimationController,
            archetype_table::{APositionTable, ArchetypeTable},
            draw_palette::{DrawData, PipelineDrawData},
            gpu_bind_registry::GPUBindRegistry,
        },
        world::{DrawSet, InstanceUploadData, NewInstanceData, RenderGroup, RenderView},
    },
};
#[cfg(test)]
use crate::{
    animation::animation::{Animation, AnimationInstance},
    util::types::GlobalTransform,
};

pub struct InstanceManager {
    pub(super) _next_id: u16,
    gpu_bind_registry: GPUBindRegistry,
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
    pub fn get_all_instances(&self) -> Vec<InstanceHandle> {
        self.gpu_bind_registry
            .registered_instances
            .values()
            .cloned()
            .collect()
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
    pub fn get_pos_table_positions(&self) -> Vec<GlobalTransform> {
        self.pos.get_positions()
    }

    #[cfg(test)]
    pub fn get_groups(&self) -> &Vec<RenderGroup> {
        &self.render_groups
    }

    pub fn register_prototype(
        &mut self,
        entity_handle: EntityHandle,
        prototype_handle: PrototypeHandle,
    ) {
        self.gpu_bind_registry
            .registered_prototypes
            .insert(entity_handle, prototype_handle);
    }

    pub fn add_draw_data(
        &mut self,
        instance_handle: &InstanceHandle,
        bindings: InstanceGPUBindings,
        gpu_instance_handle: GPUInstanceHandle,
        asset_manager: &AssetManager,
    ) {
        self.gpu_bind_registry
            .registered_instances
            .insert(gpu_instance_handle, instance_handle.clone());
        let group = &self.render_groups[self.entity_group_index[&instance_handle.entity_handle]];
        let mut data: Vec<DrawData> = Vec::new();
        for view in &group.views {
            let alloc_handle = asset_manager.resolve_asset_handle(&view.asset_handle);
            if let Some(pnu) = &view.pnu_draws {
                for (i, prim_range) in pnu.primtitive_ranges.iter().enumerate() {
                    let draw_data = DrawData::Static(PipelineDrawData {
                        alloc_handle: alloc_handle.clone(),
                        lt_idx: bindings.lt_offset + pnu.mesh_map[i],
                        joint_offset: None,
                        primitives: prim_range.clone(),
                        indices: pnu.index_ranges.as_ref().map(|x| x[i].clone()),
                    });
                    data.push(draw_data);
                }
            }
            if let Some(pnujw) = &view.pnujw_draws {
                for (i, prim_range) in pnujw.primtitive_ranges.iter().enumerate() {
                    let draw_data = DrawData::Skinned(PipelineDrawData {
                        alloc_handle: alloc_handle.clone(),
                        lt_idx: bindings.lt_offset + pnujw.mesh_map[i],
                        joint_offset: bindings
                            .joint_offset
                            .map(|offset| offset + pnujw.joint_map[i]),
                        primitives: prim_range.clone(),
                        indices: pnujw.index_ranges.as_ref().map(|x| x[i].clone()),
                    });
                    data.push(draw_data);
                }
            }
        }
        match instance_handle.archetype {
            ArchetypeId::Position => {
                self.pos.write_draw_data(instance_handle, data);
            }
        }

        // animation
        if let Some(entity_animations) = self
            .animation_controller
            .registered_animations
            .get_mut(&instance_handle.entity_handle)
        {
            entity_animations.gpu_instance_handle = Some(gpu_instance_handle);
        }
    }

    pub fn new() -> Self {
        Self {
            _next_id: 0,
            gpu_bind_registry: GPUBindRegistry::default(),
            pos: APositionTable::new(),
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
            if self.gpu_bind_registry.registered_prototypes.is_empty() {
                commands.push(command);
            } else {
                for stored_handle in self.gpu_bind_registry.registered_instances.values() {
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

    pub fn spawn_instances(
        &mut self,
        entity_manager: &EntityManager,
        instance_data: Vec<(EntityHandle, Box<dyn Archetype>)>,
    ) -> Result<Vec<InstanceUploadData>, WorldUpdateError> {
        let mut res: Vec<InstanceUploadData> = Vec::new();

        // loop through the instances being uploaded and sort them by entity handle
        let mut sorted: HashMap<EntityHandle, Vec<Box<dyn Archetype>>> = HashMap::new();
        for instance in instance_data {
            sorted
                .entry(instance.0)
                .or_insert_with(Vec::new)
                .push(instance.1);
        }
        for (entity_handle, mut arch_list) in sorted {
            // for the first instance spawn of an entity, collect asset-relative common data
            if !self.entity_group_index.contains_key(&entity_handle) {
                let first_arch = arch_list.swap_remove(0);
                let first_instance_handle = first_arch.insert_self(self, &entity_handle);
                let renderables = entity_manager
                    .get_entity_render_data(&first_instance_handle)
                    .expect("renderables fetch fail");
                let first_instance_upload_data = self.add_render_group(renderables);

                let mut additional = Vec::<InstanceHandle>::with_capacity(arch_list.len());
                // for the rest, independant data should be copied, and shared should be shared
                for arch in arch_list {
                    let instance_handle = arch.insert_self(self, &entity_handle);
                    additional.push(instance_handle);
                }
                res.push(first_instance_upload_data);

                if !additional.is_empty() {
                    let prototype_handle = self
                        .gpu_bind_registry
                        .registered_prototypes
                        .get(&entity_handle)
                        .unwrap()
                        .clone();

                    let group = &self.render_groups[self.entity_group_index[&entity_handle]];
                    let has_joints = group.views.iter().any(|v| v.pnujw_draws.is_some());
                    let copied =
                        entity_manager.get_entity_cloned(additional, prototype_handle, has_joints);
                    res.push(copied);
                }
            } else {
                let prototype_handle = self
                    .gpu_bind_registry
                    .registered_prototypes
                    .get(&entity_handle)
                    .expect("prototype should be registered")
                    .clone();

                let first_arch = arch_list.swap_remove(0);
                let first_instance_handle = first_arch.insert_self(self, &entity_handle);

                let mut handles = Vec::<InstanceHandle>::with_capacity(arch_list.len());
                handles.push(first_instance_handle);
                // for the rest, independant data should be copied, and shared should be shared
                for arch in arch_list {
                    let instance_handle = arch.insert_self(self, &entity_handle);
                    handles.push(instance_handle);
                }
                let group = &self.render_groups[self.entity_group_index[&entity_handle]];
                let has_joints = group.views.iter().any(|v| v.pnujw_draws.is_some());
                let copied_instance_data =
                    entity_manager.get_entity_cloned(handles, prototype_handle.clone(), has_joints);
                res.push(copied_instance_data);
            }
        }
        Ok(res)
    }

    pub fn add_render_group(&mut self, mut renderables: Renderables) -> InstanceUploadData {
        let prototype = self
            .gpu_bind_registry
            .gen_prototype(renderables.instance_handle.entity_handle.clone());
        let mut new_instance_data =
            NewInstanceData::new(renderables.instance_handle.clone(), prototype);
        // ******* MESH DATA ********
        let mut views = Vec::<RenderView>::with_capacity(renderables.mesh_renderables.len());

        for (asset_handle, mesh_data) in renderables.mesh_renderables.drain(..) {
            let view = RenderView {
                asset_handle: asset_handle,
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
            new_instance_data
                .local_transforms
                .extend(mesh_data.local_transforms);

            if let Some(joint_transforms) = mesh_data.joint_transforms {
                match &mut new_instance_data.joint_transforms {
                    Some(jts) => {
                        jts.extend(joint_transforms);
                        new_instance_data
                            .ibms
                            .as_mut()
                            .expect("ibms")
                            .extend(mesh_data.ibms.expect("must have ibms"));
                    }
                    None => {
                        new_instance_data.joint_transforms = Some(joint_transforms);
                        new_instance_data.ibms = Some(mesh_data.ibms.expect("must have ibms"))
                    }
                }
            }
        }

        self.entity_group_index.insert(
            renderables.instance_handle.entity_handle,
            self.render_groups.len(),
        );
        self.render_groups.push(RenderGroup { views });
        // ******** ANIMATION DATA *********
        if let Some(entity_animation_data) = renderables.animations {
            self.animation_controller.registered_animations.insert(
                renderables.instance_handle.entity_handle.clone(),
                EntityAnimations {
                    gpu_instance_handle: None,
                    animation: entity_animation_data.animation,
                    local_transforms: entity_animation_data.local_transforms,
                    joint_transforms: entity_animation_data.joint_transforms,
                    mesh_slot_map: entity_animation_data.mesh_slot_map,
                    skin_offset_map: entity_animation_data.skin_offset_map,
                },
            );
        }
        InstanceUploadData::New(new_instance_data)
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
        // TODO: could probably save some time by just clearing the vecs, not the key value
        packet.clear();
        // pos
        for (instance_idx, draw_chunk) in self.pos.draw_palette.iter().enumerate() {
            for data in draw_chunk.iter() {
                let entry = match data {
                    DrawData::Static(pnu_data) => packet
                        .pnu
                        .entry(pnu_data.alloc_handle.clone())
                        .or_insert_with(Vec::new),
                    DrawData::Skinned(pnujw_data) => packet
                        .pnujw
                        .entry(pnujw_data.alloc_handle.clone())
                        .or_insert_with(Vec::new),
                };
                entry.push(data.as_draw_item(instance_idx as u32));
            }
        }
    }

    pub fn prepare_render_frame<'frame>(&'frame self) -> RenderFrame<'frame> {
        let mut render_frame = RenderFrame::default();
        self.pos.collect(&mut render_frame);

        self.animation_controller
            .prepare_animation_frame(&mut render_frame);
        render_frame
    }
}
