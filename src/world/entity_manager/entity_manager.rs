use std::{any::type_name, collections::HashSet, mem::MaybeUninit, ops::Range};

use crate::{
    asset_manager::{
        AssetHandle, ProvidesAnimationData, ProvidesMaterialData, ProvidesMeshData,
        asset_manager::AssetManager,
    },
    common::{entity::EntityHandle, instance::InstanceHandle},
    renderer::PrototypeHandle,
    util::types::{InverseBindMatrix, JointTransform, LocalTransform},
    world::{
        entity_manager::{
            EntityManagerError, Renderables,
            components::{
                AnimationComponent, AnimationMode, Component, MaterialPalleteComponent,
                MeshCollectionComponent, MeshCollectionDescriptor,
            },
        },
        world::{
            CopiedInstanceData, InstanceUploadData, InverseBindMatrices, JointTransforms,
            LocalTransforms, NewInstanceData,
        },
    },
};

pub struct EntityManager {
    available_ids: Vec<std::range::Range<u32>>,
    prototypes: SparseSet<PrototypeHandle, 100>,
    mesh_collections: SparseSet<MeshCollectionComponent<dyn ProvidesMeshData>, 100>,
    materials: SparseSet<MaterialPalleteComponent<dyn ProvidesMaterialData>, 100>,
    animations: SparseSet<AnimationComponent<dyn ProvidesAnimationData>, 100>,
}

impl EntityManager {
    pub fn release_prototype(&mut self, entity_handle: &EntityHandle) -> Option<PrototypeHandle> {
        self.prototypes.remove(entity_handle.0 as usize)
    }
    pub fn prototype_of(&self, entity_handle: &EntityHandle) -> Option<PrototypeHandle> {
        self.prototypes.get(entity_handle.0 as usize).cloned()
    }
    pub fn ack_prototype(
        &mut self,
        entity_handle: &EntityHandle,
        prototype_handle: PrototypeHandle,
    ) {
        self.prototypes
            .insert(entity_handle.0 as usize, prototype_handle);
    }

    pub fn get_entity_new<'frame>(
        &'frame self,
        instance_handle: &InstanceHandle,
        local_transform_data: Vec<LocalTransform>,
        joint_transform_data: Option<Vec<JointTransform>>,
        ibm_data: Option<Vec<InverseBindMatrix>>,
    ) -> NewInstanceData {
        let anim = self
            .animations
            .get(instance_handle.entity_handle.0 as usize);
        let rigid_mode = anim
            .map(|a| &a.rigid_animation_mode)
            .unwrap_or(&AnimationMode::None);
        let skinned_mode = anim
            .map(|a| &a.skinned_animation_mode)
            .unwrap_or(&AnimationMode::None);
        let local_transforms = match rigid_mode {
            AnimationMode::Independent => LocalTransforms::OwnedCopy {
                data: local_transform_data,
            },
            AnimationMode::Shared | AnimationMode::None => LocalTransforms::OwnedShared {
                data: local_transform_data,
            },
        };
        let (joint_transforms, ibms) = if let Some(joints) = joint_transform_data {
            let jt_res = match skinned_mode {
                AnimationMode::Shared | AnimationMode::None => {
                    JointTransforms::OwnedShared { data: joints }
                }
                AnimationMode::Independent => JointTransforms::OwnedCopy { data: joints },
            };
            let ibm_res = InverseBindMatrices::Owned {
                data: ibm_data.expect("must have ibms"),
            };
            (jt_res, ibm_res)
        } else {
            (JointTransforms::None, InverseBindMatrices::None)
        };

        NewInstanceData {
            handle: instance_handle.clone(),
            local_transforms,
            joint_transforms,
            ibms,
            additional: Vec::new(),
        }
    }
    pub fn get_entity_cloned<'frame>(
        &'frame self,
        instance_handles: Vec<InstanceHandle>,
        prototype_handle: PrototypeHandle,
        has_joints: bool,
    ) -> InstanceUploadData {
        let anim = self
            .animations
            .get(instance_handles[0].entity_handle.0 as usize);
        let rigid_mode = anim
            .map(|a| &a.rigid_animation_mode)
            .unwrap_or(&AnimationMode::None);
        let skinned_mode = anim
            .map(|a| &a.skinned_animation_mode)
            .unwrap_or(&AnimationMode::None);
        let local_transforms = match rigid_mode {
            AnimationMode::Shared | AnimationMode::None => LocalTransforms::NeedsShared,
            AnimationMode::Independent => LocalTransforms::NeedsCopy,
        };
        let joint_transforms = if has_joints {
            match skinned_mode {
                AnimationMode::Shared | AnimationMode::None => JointTransforms::NeedsShared,
                AnimationMode::Independent => JointTransforms::NeedsCopy,
            }
        } else {
            JointTransforms::None
        };

        let copied = CopiedInstanceData {
            handles: instance_handles,
            prototype_handle,
            local_transforms,
            joint_transforms,
        };

        InstanceUploadData::Copied(copied)
    }

    pub(crate) fn get_entity_render_data<'frame>(
        &'frame self,
        instance_handle: &InstanceHandle,
        asset_manager: &AssetManager,
    ) -> Result<Renderables, EntityManagerError> {
        let mut renderables = Renderables {
            instance_handle: instance_handle.clone(),
            mesh_renderables: Vec::new(),
            animations: None,
            material_palette: Vec::new(),
        };

        if let Some(mesh_collection) = self
            .mesh_collections
            .get(instance_handle.entity_handle.0 as usize)
        {
            let loaded_asset =
                asset_manager.get_loaded_asset(&mesh_collection.resource_backing.asset_handle);

            let mesh_renderables =
                mesh_collection.get_output_data(loaded_asset.as_mesh_provider().unwrap());

            renderables
                .mesh_renderables
                .push((loaded_asset.alloc_handle().clone(), mesh_renderables));
        }
        if let Some(animation_component) = self
            .animations
            .get(instance_handle.entity_handle.0 as usize)
        {
            let asset =
                asset_manager.get_loaded_asset(&animation_component.resource_backing.asset_handle);

            let entity_animations =
                animation_component.get_output_data(asset.as_animation_provider().unwrap());
            renderables.animations = Some(entity_animations);
        }

        if let Some(materials_component) =
            self.materials.get(instance_handle.entity_handle.0 as usize)
        {
            let asset =
                asset_manager.get_loaded_asset(&materials_component.resource_backing.asset_handle);
            let indices =
                materials_component.get_output_data(asset.as_materials_provider().unwrap());
            let alloc = asset_manager
                .alloc_handle_of(&materials_component.resource_backing.asset_handle)
                .unwrap();
            renderables.material_palette.push(Some((alloc, indices)));
        } else {
            renderables.material_palette.push(None);
        }
        Ok(renderables)
    }

    pub fn rbcs_of(
        &self,
        entity_handle: EntityHandle,
        asset_manager: &AssetManager,
    ) -> HashSet<AssetHandle> {
        let mut result = HashSet::<AssetHandle>::new();
        if let Some(mesh_collection_component) = self.mesh_collections.get(entity_handle.0 as usize)
        {
            result.insert(mesh_collection_component.resource_backing.asset_handle);
            if let Some(deps) = asset_manager
                .external_dependencies_of(&mesh_collection_component.resource_backing.asset_handle)
            {
                for dep in deps {
                    result.insert(dep);
                }
            }
        }
        if let Some(material_palette_component) = self.materials.get(entity_handle.0 as usize) {
            result.insert(material_palette_component.resource_backing.asset_handle);
            if let Some(deps) = asset_manager
                .external_dependencies_of(&material_palette_component.resource_backing.asset_handle)
            {
                for dep in deps {
                    result.insert(dep);
                }
            }
        }
        return result;
    }

    pub fn new_entity(&mut self) -> Result<EntityHandle, EntityManagerError> {
        // return the lowest number available
        let first_range = self
            .available_ids
            .first_mut()
            .ok_or(EntityManagerError::MaxEntitiesExceeded)?;
        let res = EntityHandle(first_range.start as u16);
        if first_range.end - first_range.start > 1 {
            first_range.start = first_range.start + 1;
        } else {
            self.available_ids.remove(0);
        }
        return Ok(res);
    }

    pub fn new() -> Self {
        Self {
            available_ids: vec![Range::from(0..10000).into()],
            mesh_collections: SparseSet::new(),
            animations: SparseSet::new(),
            materials: SparseSet::new(),
            prototypes: SparseSet::new(),
        }
    }

    pub fn add_mesh_collection_for_entity(
        &mut self,
        entity: &EntityHandle,
        descriptor: MeshCollectionDescriptor,
    ) {
        let mcc = MeshCollectionComponent {
            mesh_accessor: descriptor.mesh_accessor,
            resource_backing: descriptor.resource_backing,
        };
        self.mesh_collections.insert(entity.0 as usize, mcc.erase());
        if let Some(animation) = descriptor.animation {
            self.animations.insert(entity.0 as usize, animation.erase());
        }
        if let Some(material) = descriptor.materials {
            self.materials.insert(entity.0 as usize, material.erase());
        }
    }

    pub fn add_animation_for_entity<A>(
        &mut self,
        entity: &EntityHandle,
        animation: AnimationComponent<A>,
    ) where
        A: ProvidesAnimationData,
    {
        self.animations.insert(entity.0 as usize, animation.erase());
    }

    pub fn add_material_for_entity<M>(
        &mut self,
        entity_handle: &EntityHandle,
        material_component: MaterialPalleteComponent<M>,
    ) where
        M: ProvidesMaterialData,
    {
        self.materials
            .insert(entity_handle.0 as usize, material_component.erase());
    }

    #[cfg(test)]
    pub fn get_registered_prototypes(&self) -> Vec<PrototypeHandle> {
        let mut res = Vec::new();
        for s in self.prototypes.sparse.iter().filter(|s| **s != INVALID) {
            unsafe {
                res.push(self.prototypes.dense[*s].assume_init().clone());
            }
        }
        res
    }
}

const INVALID: usize = usize::MAX;
pub(super) struct SparseSet<T, const N: usize> {
    dense: [MaybeUninit<T>; N],
    pub(super) dense_ids: [usize; N],
    pub(super) sparse: [usize; N],
    pub(super) len: usize,
}

impl<T, const N: usize> SparseSet<T, N> {
    pub fn new() -> Self {
        Self {
            dense: unsafe { MaybeUninit::uninit().assume_init() },
            dense_ids: [INVALID; N],
            sparse: [INVALID; N],
            len: 0,
        }
    }

    pub fn insert(&mut self, id: usize, value: T) {
        assert!(self.len + 1 < N, "SparseSet is full");
        assert!(id < N, "ID out of bounds");

        if self.contains(id) {
            panic!("ID already present in SparseSet, {:?}", type_name::<T>());
        }

        let dense_index = self.len;

        // write value
        self.dense[dense_index].write(value);
        self.dense_ids[dense_index] = id;
        self.sparse[id] = dense_index;

        self.len += 1;
    }

    pub(super) fn get(&self, id: usize) -> Option<&T> {
        if self.contains(id) {
            unsafe { return Some(self.dense[self.sparse[id]].assume_init_ref()) }
        }
        None
    }

    #[allow(unused)]
    pub(super) fn get_mut(&mut self, id: usize) -> Option<&mut T> {
        if self.contains(id) {
            unsafe {
                return Some(self.dense[self.sparse[id]].assume_init_mut());
            }
        }
        None
    }

    #[inline]
    pub fn contains(&self, id: usize) -> bool {
        id < N && self.sparse[id] < self.len && self.dense_ids[self.sparse[id]] == id
    }

    pub fn remove(&mut self, id: usize) -> Option<T> {
        if !self.contains(id) {
            return None;
        }

        let dense_index = self.sparse[id];
        let last_index = self.len - 1;

        let removed = unsafe { self.dense[dense_index].assume_init_read() };

        if dense_index != last_index {
            let moved_id = self.dense_ids[last_index];
            let moved_value = unsafe { self.dense[last_index].assume_init_read() };
            self.dense[dense_index].write(moved_value);
            self.dense_ids[dense_index] = moved_id;
            self.sparse[moved_id] = dense_index;
        }

        self.dense_ids[last_index] = INVALID;
        self.sparse[id] = INVALID;
        self.len -= 1;

        Some(removed)
    }
}
