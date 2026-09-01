use std::{collections::HashSet, mem::MaybeUninit, ops::Range};

use crate::{
    asset_manager::{
        AssetHandle, ProvidesAnimationData, ProvidesMeshData, asset_manager::AssetManager,
    },
    common::{entity::EntityHandle, instance::InstanceHandle},
    renderer::PrototypeHandle,
    world::{
        entity_manager::{
            EntityManagerError, Renderables,
            components::{
                AnimationComponent, AnimationMode, Component, MeshCollectionComponent,
                MeshCollectionDescriptor,
            },
        },
        world::{CopiedInstanceData, InstanceUploadData, JointTransforms, LocalTransforms},
    },
};

pub struct EntityManager {
    available_ids: Vec<std::range::Range<u32>>,
    mesh_collections: SparseSet<MeshCollectionComponent<dyn ProvidesMeshData>, 100>,
    animations: SparseSet<AnimationComponent<dyn ProvidesAnimationData>, 100>,
}

impl EntityManager {
    pub fn get_entity_cloned<'frame>(
        &'frame self,
        instance_handles: Vec<InstanceHandle>,
        prototype_handle: PrototypeHandle,
        has_joints: bool,
    ) -> InstanceUploadData {
        let mut local_transforms = LocalTransforms::NeedsShared;
        let mut joint_transforms = JointTransforms::None;

        if let Some(anim) = self
            .animations
            .get(instance_handles.get(0).as_ref().unwrap().entity_handle.0 as usize)
        {
            match anim.rigid_animation_mode {
                AnimationMode::Shared => local_transforms = LocalTransforms::NeedsShared,
                AnimationMode::Independent => local_transforms = LocalTransforms::NeedsCopy,
                AnimationMode::None => local_transforms = LocalTransforms::NeedsShared,
            }
            joint_transforms = if has_joints {
                match anim.skinned_animation_mode {
                    AnimationMode::Shared => JointTransforms::NeedsShared,
                    AnimationMode::Independent => JointTransforms::NeedsCopy,
                    AnimationMode::None => JointTransforms::None,
                }
            } else {
                JointTransforms::None
            };
        }
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

        Ok(renderables)
    }

    pub fn rbcs_of(&self, entity_handle: EntityHandle) -> HashSet<AssetHandle> {
        let mut result = HashSet::<AssetHandle>::new();
        if let Some(mesh_collection_component) = self.mesh_collections.get(entity_handle.0 as usize)
        {
            result.insert(mesh_collection_component.resource_backing.asset_handle);
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
            panic!("ID already present in SparseSet");
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
}
