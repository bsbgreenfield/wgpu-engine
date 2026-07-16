use std::collections::HashMap;

use crate::{
    animation::EntityAnimations,
    asset_manager::asset_manager::AssetManager,
    common::{
        entity::{EntityHandle, PrototypeHandle},
        instance::InstanceHandle,
    },
    world::{
        WorldUpdateError,
        entity_manager::{Renderables, entity_manager::EntityManager},
        instance_manager::{archetypes::Archetype, instance_manager::InstanceManager},
        world::{DrawSet, InstanceUploadData, NewInstanceData, RenderGroup, RenderView},
    },
};

impl InstanceManager {
    pub fn spawn_instances(
        &mut self,
        entity_manager: &EntityManager,
        asset_manager: &AssetManager,
        instance_data: Vec<(EntityHandle, Box<dyn Archetype>)>,
    ) -> Result<Vec<InstanceUploadData>, WorldUpdateError> {
        let mut res: Vec<InstanceUploadData> = Vec::new();
        let sorted = Self::sort_entities(instance_data);

        for (entity_handle, arch_list) in sorted {
            let registered: bool = self
                .gpu_bind_registry
                .registered_prototypes
                .contains_key(&entity_handle);

            if registered {
                let handles = self.insert_archetypes(&entity_handle, arch_list);
                let upload_data = self.copy_instances(entity_manager, &entity_handle, handles);
                res.push(upload_data);
            } else {
                let (new_instance_data, additional) =
                    self.spawn_new_entity(entity_manager, asset_manager, entity_handle, arch_list);
                res.push(InstanceUploadData::New(new_instance_data));
                if !additional.is_empty() {
                    res.push(self.copy_instances(entity_manager, &entity_handle, additional));
                }
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
    ) -> (NewInstanceData, Vec<InstanceHandle>) {
        // take the first instance so that a prototype can be generated from it
        let first_arch = arch_list.swap_remove(0);
        let first_instance_handle = first_arch.insert_self(self, &entity_handle);

        // get renderable data from entity maanger
        let mut renderables = entity_manager
            .get_entity_render_data(&first_instance_handle, asset_manager)
            .expect("renderables fetch fail");

        // create prototype
        let prototype = self
            .gpu_bind_registry
            .gen_prototype(renderables.instance_handle.entity_handle.clone());

        // simlutaneously generate the render group of the new entity and the new instance upload data
        let (render_group, new_instance_data) = Self::new_instance(&mut renderables, prototype);

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
        (new_instance_data, additional)
    }

    pub fn new_instance(
        renderables: &mut Renderables,
        prototype: PrototypeHandle,
    ) -> (RenderGroup, NewInstanceData) {
        let mut new_instance_data =
            NewInstanceData::new(renderables.instance_handle.clone(), prototype);
        let mut views = Vec::<RenderView>::with_capacity(renderables.mesh_renderables.len());
        for (alloc_handle, mesh_data) in renderables.mesh_renderables.drain(..) {
            let view = RenderView {
                alloc_handle: alloc_handle,
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

        (
            RenderGroup {
                views,
                entity_handle: renderables.instance_handle.entity_handle,
            },
            new_instance_data,
        )
    }

    pub fn copy_instances(
        &self,
        entity_manager: &EntityManager,
        entity_handle: &EntityHandle,
        handles: Vec<InstanceHandle>,
    ) -> InstanceUploadData {
        let prototype_handle = self
            .gpu_bind_registry
            .registered_prototypes
            .get(&entity_handle)
            .expect("prototype should be registered")
            .clone();

        let has_joints = self.group_has_joints(entity_handle);
        entity_manager.get_entity_cloned(handles, prototype_handle, has_joints)
    }

    pub(super) fn sort_entities(
        instance_data: Vec<(EntityHandle, Box<dyn Archetype>)>,
    ) -> HashMap<EntityHandle, Vec<Box<dyn Archetype>>> {
        // loop through the instances being uploaded and sort them by entity handle
        let mut sorted: HashMap<EntityHandle, Vec<Box<dyn Archetype>>> = HashMap::new();
        for instance in instance_data {
            sorted
                .entry(instance.0)
                .or_insert_with(Vec::new)
                .push(instance.1);
        }
        sorted
    }

    fn group_has_joints(&self, entity_handle: &EntityHandle) -> bool {
        let group = &self.render_groups[self.sparse_entity_group[entity_handle.0 as usize]];
        group.views.iter().any(|v| v.pnujw_draws.is_some())
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

    pub fn push_render_group(&mut self, render_group: RenderGroup, renderables: &Renderables) {
        if self.sparse_entity_group.len() < renderables.instance_handle.entity_handle.0 as usize {
            self.sparse_entity_group.resize(
                renderables.instance_handle.entity_handle.0 as usize,
                usize::MAX,
            );
        }
        self.sparse_entity_group[renderables.instance_handle.entity_handle.0 as usize] =
            self.render_groups.len();
        self.render_groups.push(render_group);
    }
}

impl InstanceSpawn for InstanceManager {}
pub trait InstanceSpawn {
    fn get_entity_animations(renderables: Renderables) -> Option<EntityAnimations> {
        if let Some(entity_animation_data) = renderables.animations {
            return Some(EntityAnimations {
                gpu_instance_handle: None,
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
