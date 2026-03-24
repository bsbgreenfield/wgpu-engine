use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Display,
    mem::MaybeUninit,
};

use crate::{
    app::renderer_new::GPUAllocationHandle,
    asset_manager::asset_manager::AssetHandle,
    world::components::{
        ComponentData, ComponentDataType, MeshCollectionComponent, PhysicalPositionComponent,
    },
};

#[derive(Debug)]
pub enum EntityManagerError {
    MaxEntitiesExceeded,
    InvalidInitialization(ComponentDataType),
}
impl Display for EntityManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return f.write_str(&self.to_string());
    }
}
impl Error for EntityManagerError {}

pub struct EntityManager {
    available_ids: Vec<std::ops::Range<u32>>,
    mesh_collections: SparseSet<MeshCollectionComponent, 100>,
    global_transforms: SparseSet<PhysicalPositionComponent, 100>,
}

pub struct Renderables<'frame> {
    pub mesh_collection: Option<&'frame MeshCollectionComponent>,
}

impl EntityManager {
    pub fn component_data_types_of(&self, entity: &EntityHandle) -> Vec<ComponentDataType> {
        let mut res = Vec::new();
        if self.global_transforms.get(entity.0 as usize).is_some() {
            res.push(ComponentDataType::PhysicalPosition);
        }
        res
    }

    pub fn get_renderables<'frame>(&'frame self, entity: &EntityHandle) -> Renderables<'frame> {
        Renderables {
            mesh_collection: self.mesh_collections.get(entity.0 as usize),
        }
    }

    pub(super) fn saturate_rbcs(
        &mut self,
        entity: EntityHandle,
        allocation_handles: HashMap<AssetHandle, GPUAllocationHandle>,
    ) {
        if let Some(mcc) = self.mesh_collections.get_mut(entity.0 as usize)
            && let Some(alloc_handle) = allocation_handles.get(&mcc.resource_backing)
        {
            mcc.allocation_handle.insert(alloc_handle.clone()); // should this be Weak?
        }
        // TODO: saturate other rbcs
    }

    pub fn unallocated_assets_of(&self, entity_handle: EntityHandle) -> HashSet<AssetHandle> {
        let mut result = HashSet::<AssetHandle>::new();
        if let Some(mesh_collection_component) = self.mesh_collections.get(entity_handle.0 as usize)
            && mesh_collection_component.allocation_handle.is_none()
        {
            result.insert(mesh_collection_component.resource_backing);
        }
        return result;
    }

    pub fn new_entity(&mut self) -> Result<EntityHandle, EntityManagerError> {
        // return the lowest number available
        let first_range = self
            .available_ids
            .first_mut()
            .ok_or(EntityManagerError::MaxEntitiesExceeded)?;
        let res = EntityHandle(first_range.start);
        if first_range.len() > 1 {
            first_range.start = first_range.start + 1;
        } else {
            self.available_ids.remove(0);
        }
        return Ok(res);
    }

    pub fn new() -> Self {
        Self {
            available_ids: vec![],
            mesh_collections: SparseSet::new(),
            global_transforms: SparseSet::new(),
        }
    }

    pub fn add_mesh_collection_for_entity(
        &mut self,
        entity: EntityHandle,
        mesh_collection: MeshCollectionComponent,
    ) {
        self.mesh_collections
            .insert(entity.0 as usize, mesh_collection);
    }

    pub fn add_physical_position_for_entity(&mut self, entity: EntityHandle) {
        self.global_transforms
            .insert(entity.0 as usize, PhysicalPositionComponent {});
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityHandle(u32);
pub struct Entity {
    handle: EntityHandle,
}
const INVALID: usize = usize::MAX;
struct SparseSet<T, const N: usize> {
    dense: [MaybeUninit<T>; N],
    dense_ids: [usize; N],
    sparse: [usize; N],
    len: usize,
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
        assert!(id < N, "ID out of bounds");
        assert!(self.len < N, "SparseSet is full");

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

    fn get(&self, id: usize) -> Option<&T> {
        if self.contains(id) {
            unsafe { return Some(self.dense[self.sparse[id]].assume_init_ref()) }
        }
        None
    }

    fn get_mut(&mut self, id: usize) -> Option<&mut T> {
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
