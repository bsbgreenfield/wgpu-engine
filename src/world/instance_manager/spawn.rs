use std::{collections::HashMap, iter::repeat_n};

use crate::{
    animation::EntityAnimations,
    asset_manager::asset_manager::AssetManager,
    common::{entity::EntityHandle, instance::InstanceHandle},
    world::{
        WorldUpdateError,
        entity_manager::{Renderables, entity_manager::EntityManager},
        instance_manager::{archetypes::Archetype, instance_manager::InstanceManager},
        scene::scene::Spawn,
        world::{DrawSet, InstanceUploadData, NewInstanceData, RenderGroup, RenderView},
    },
};

impl InstanceManager {
    pub fn spawn_instances(
        &mut self,
        entity_manager: &EntityManager,
        asset_manager: &AssetManager,
        instance_data: Vec<Spawn<dyn Archetype>>,
    ) -> Result<Vec<InstanceUploadData>, WorldUpdateError> {
        let mut res: Vec<InstanceUploadData> = Vec::new();
        let sorted = Self::sort_entities(instance_data);

        for (entity_handle, arch_list) in sorted {
            let registered: bool = entity_manager.prototype_of(&entity_handle).is_some();

            if registered {
                let handles = self.insert_archetypes(&entity_handle, arch_list);
                let upload_data = self.copy_instances(entity_manager, &entity_handle, handles);
                res.push(upload_data);
            } else {
                let new_instance_data =
                    self.spawn_new_entity(entity_manager, asset_manager, entity_handle, arch_list);
                res.push(InstanceUploadData::New(new_instance_data));
            }
        }

        Ok(res)
    }
    fn spawn_new_entity(
        &mut self,
        entity_manager: &EntityManager,
        asset_manager: &AssetManager,
        entity_handle: EntityHandle,
        mut arch_list: Vec<Box<dyn Archetype>>,
    ) -> NewInstanceData {
        // take the first instance so that a prototype can be generated from it
        let first_arch = arch_list.swap_remove(0);
        let first_instance_handle = first_arch.insert_self(self, &entity_handle);

        // get renderable data from entity maanger
        let mut renderables = entity_manager
            .get_entity_render_data(&first_instance_handle, asset_manager)
            .expect("renderables fetch fail");

        // simlutaneously generate the render group of the new entity and the new instance upload data
        let (render_group, mut new_instance_data) =
            Self::new_instance(entity_manager, &mut renderables);

        self.push_render_group(render_group, &renderables);

        // extract animation data, if relevant
        if let Some(entity_animations) = Self::get_entity_animations(renderables) {
            self.animation_controller.registered_animations.insert(
                first_instance_handle.entity_handle.clone(),
                entity_animations,
            );
        }

        // insert all other instances into archetype tables
        let additional = self.insert_archetypes(&entity_handle, arch_list);
        new_instance_data.additional = additional;
        new_instance_data
    }

    pub(super) fn new_instance(
        entity_manager: &EntityManager,
        renderables: &mut Renderables,
    ) -> (RenderGroup, NewInstanceData) {
        let mut views = Vec::<RenderView>::with_capacity(renderables.mesh_renderables.len());
        // TODO: change raw u32 to a structure in which a GPUAllocHandle can be included for an
        // external material
        //
        let mut new_instance_data: Option<NewInstanceData> = None;
        for ((alloc_handle, mesh_data), maybe_material) in renderables
            .mesh_renderables
            .drain(..)
            .zip(renderables.material_palette.drain(..))
        {
            let view = RenderView {
                alloc_handle: alloc_handle,
                pnu_draws: mesh_data.pnu_vertex_ranges.map(|pnu| DrawSet {
                    joint_map: vec![], // TODO: seprate draw set struct for pnu to avoid this?
                    mesh_map: mesh_data.pnu_mesh_map,
                    primtitive_ranges: pnu,
                    index_ranges: mesh_data.pnu_index_ranges.clone(),
                    material_indices: maybe_material
                        .as_ref()
                        .map(|(_material_alloc, material_indices)| {
                            mesh_data
                                .pnu_materials
                                .iter()
                                .map(|primitive_mat_idx| {
                                    primitive_mat_idx.map(|i| material_indices[i as usize])
                                })
                                .collect()
                        })
                        .unwrap_or(Vec::from_iter(repeat_n(
                            None,
                            mesh_data.pnu_materials.len(),
                        ))),
                }),
                pnujw_draws: mesh_data.pnujw_vertex_ranges.map(|pnujw| DrawSet {
                    joint_map: mesh_data.joint_map,
                    mesh_map: mesh_data.pnujw_mesh_map,
                    primtitive_ranges: pnujw,
                    index_ranges: mesh_data.pnujw_index_ranges.clone(),
                    material_indices: maybe_material
                        .as_ref()
                        .map(|(_material_alloc, material_indices)| {
                            mesh_data
                                .pnujw_materials
                                .iter()
                                .map(|primitive_mat_idx| {
                                    primitive_mat_idx.map(|i| material_indices[i as usize])
                                })
                                .collect()
                        })
                        .unwrap_or(Vec::from_iter(repeat_n(
                            None,
                            mesh_data.pnujw_materials.len(),
                        ))),
                }),
            };

            views.push(view);
            new_instance_data = Some(entity_manager.get_entity_new(
                &renderables.instance_handle,
                mesh_data.local_transforms,
                mesh_data.joint_transforms,
                mesh_data.ibms,
            ));
        }

        (
            RenderGroup::new(views, renderables.instance_handle.entity_handle),
            new_instance_data.expect("there must be at least one new instance data"),
        )
    }

    pub fn copy_instances(
        &self,
        entity_manager: &EntityManager,
        entity_handle: &EntityHandle,
        handles: Vec<InstanceHandle>,
    ) -> InstanceUploadData {
        let prototype_handle = entity_manager
            .prototype_of(entity_handle)
            .expect("prototype should be registered");

        let has_joints = self.group_has_joints(entity_handle);
        entity_manager.get_entity_cloned(handles, prototype_handle, has_joints)
    }

    pub(super) fn sort_entities(
        instance_data: Vec<Spawn<dyn Archetype>>,
    ) -> HashMap<EntityHandle, Vec<Box<dyn Archetype>>> {
        // loop through the instances being uploaded and sort them by entity handle
        let mut sorted: HashMap<EntityHandle, Vec<Box<dyn Archetype>>> = HashMap::new();
        for instance in instance_data {
            sorted
                .entry(instance.entity)
                .or_insert_with(Vec::new)
                .push(instance.data);
        }
        sorted
    }

    fn group_has_joints(&self, entity_handle: &EntityHandle) -> bool {
        let slot = self.sparse_entity_group[entity_handle.0 as usize];
        self.render_groups
            .get(slot)
            .and_then(|group| group.as_ref())
            .map(|g| g.views().iter().any(|v| v.pnujw_draws.is_some()))
            .unwrap_or(false)
    }

    pub fn insert_archetypes(
        &mut self,
        entity_handle: &EntityHandle,
        arch_list: Vec<Box<dyn Archetype>>,
    ) -> Vec<InstanceHandle> {
        arch_list
            .into_iter()
            .map(|arch| arch.insert_self(self, entity_handle))
            .collect()
    }

    pub(super) fn push_render_group(
        &mut self,
        render_group: RenderGroup,
        renderables: &Renderables,
    ) {
        let entity_id = renderables.instance_handle.entity_handle.0 as usize;
        if self.sparse_entity_group.len() < entity_id {
            self.sparse_entity_group.resize(entity_id + 1, usize::MAX);
        }
        let slot = match self.free_group_slots.pop() {
            Some(slot) => {
                self.render_groups[slot] = Some(render_group);
                slot
            }
            None => {
                self.render_groups.push(Some(render_group));
                self.render_groups.len() - 1
            }
        };
        self.sparse_entity_group[entity_id] = slot;
    }
}

impl InstanceSpawn for InstanceManager {}
pub trait InstanceSpawn {
    fn get_entity_animations(renderables: Renderables) -> Option<EntityAnimations> {
        if let Some(entity_animation_data) = renderables.animations {
            return Some(EntityAnimations {
                animation: entity_animation_data.animation,
                local_transforms: entity_animation_data.local_transforms,
                joint_transforms: entity_animation_data.joint_transforms,
                mesh_slot_map: entity_animation_data.mesh_slot_map,
                skin_offset_map: entity_animation_data.skin_offset_map,
            });
        } else {
            None
        }
    }
}
