use std::{collections::HashSet, error::Error, fmt::Display, mem::MaybeUninit, ops::Range};

use crate::{
    app::renderer::GPUAllocationHandle,
    asset_manager_new::{AssetHandle, asset_manager_new::AssetManagerNew},
    world::components::{ComponentDataType, MeshCollectionComponent, PhysicalPositionComponent},
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

#[derive(Debug)]
pub enum InstanceRenderData {
    MeshRenderable {
        gpu_alloc_handle: GPUAllocationHandle,
        pnu_vertex_ranges: Option<Vec<Range<u32>>>,
        pnujw_vertex_ranges: Option<Vec<Range<u32>>>,
        index_ranges: Option<Vec<Range<u32>>>,
    },
}

#[derive(Debug)]
pub struct Renderables(pub Vec<InstanceRenderData>);

impl EntityManager {
    pub fn component_data_types_of(&self, entity: &EntityHandle) -> Vec<ComponentDataType> {
        let mut res = Vec::new();
        if self.global_transforms.get(entity.0 as usize).is_some() {
            res.push(ComponentDataType::PhysicalPosition);
        }
        res
    }

    pub fn get_renderables<'frame>(
        &'frame self,
        entity: &EntityHandle,
        asset_manager: &AssetManagerNew,
    ) -> Option<Renderables> {
        let mesh_collection = self.mesh_collections.get(entity.0 as usize).unwrap();
        let instance_render_data = asset_manager.get_renderables_for(mesh_collection);
        // TODO: collect other instance render data
        if instance_render_data.is_empty() {
            return None;
        }
        Some(Renderables(instance_render_data))
    }

    pub fn rbcs_of(&self, entity_handle: EntityHandle) -> HashSet<AssetHandle> {
        let mut result = HashSet::<AssetHandle>::new();
        if let Some(mesh_collection_component) = self.mesh_collections.get(entity_handle.0 as usize)
        {
            result.insert(mesh_collection_component.resource_backing);
        }
        // TODO: other RBCs
        return result;
    }

    pub fn new_entity(&mut self) -> Result<EntityHandle, EntityManagerError> {
        // return the lowest number available
        let first_range = self
            .available_ids
            .first_mut()
            .ok_or(EntityManagerError::MaxEntitiesExceeded)?;
        let res = EntityHandle(first_range.start as u16);
        if first_range.len() > 1 {
            first_range.start = first_range.start + 1;
        } else {
            self.available_ids.remove(0);
        }
        return Ok(res);
    }

    pub fn new() -> Self {
        Self {
            available_ids: vec![0..10000],
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
pub struct EntityHandle(pub u16);

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

    fn get(&self, id: usize) -> Option<&T> {
        if self.contains(id) {
            unsafe { return Some(self.dense[self.sparse[id]].assume_init_ref()) }
        }
        None
    }

    #[allow(unused)]
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
#[cfg(test)]
mod sparse_set_tests {
    use super::*;

    type TestSet = SparseSet<i32, 8>;

    #[test]
    fn insert_and_get() {
        let mut set = TestSet::new();

        set.insert(3, 42);

        assert!(set.contains(3));
        assert_eq!(set.get(3), Some(&42));
    }

    #[test]
    fn insert_multiple() {
        let mut set = TestSet::new();

        set.insert(1, 10);
        set.insert(4, 20);
        set.insert(6, 30);

        assert_eq!(set.get(1), Some(&10));
        assert_eq!(set.get(4), Some(&20));
        assert_eq!(set.get(6), Some(&30));
    }

    #[test]
    fn contains_false_when_not_present() {
        let mut set = TestSet::new();

        set.insert(2, 99);

        assert!(!set.contains(1));
        assert!(!set.contains(7));
    }

    #[test]
    fn get_returns_none_when_not_present() {
        let set = TestSet::new();

        assert_eq!(set.get(0), None);
    }

    #[test]
    #[should_panic(expected = "ID already present")]
    fn insert_duplicate_panics() {
        let mut set = TestSet::new();

        set.insert(2, 10);
        set.insert(2, 20); // should panic
    }

    #[test]
    #[should_panic(expected = "ID out of bounds")]
    fn insert_out_of_bounds_panics() {
        let mut set = TestSet::new();

        set.insert(100, 1);
    }

    #[test]
    #[should_panic(expected = "SparseSet is full")]
    fn insert_when_full_panics() {
        let mut set = SparseSet::<i32, 2>::new();

        set.insert(0, 1);
        set.insert(1, 2);

        // third insert should panic
        set.insert(2, 3);
    }

    #[test]
    fn dense_is_compact() {
        let mut set = TestSet::new();

        set.insert(5, 50);
        set.insert(2, 20);
        set.insert(7, 70);

        // Ensure elements are packed in dense[0..len]
        for i in 0..set.len {
            let id = set.dense_ids[i];
            assert!(set.contains(id));
            assert_eq!(set.sparse[id], i);
        }
    }

    #[test]
    fn get_mut_allows_modification() {
        let mut set = TestSet::new();

        set.insert(3, 10);

        if let Some(v) = set.get_mut(3) {
            *v = 99;
        }

        assert_eq!(set.get(3), Some(&99));
    }

    #[test]
    fn sparse_and_dense_consistency() {
        let mut set = TestSet::new();

        for i in 0..5 {
            set.insert(i, i as i32 * 10);
        }

        for id in 0..5 {
            assert!(set.contains(id));

            let dense_index = set.sparse[id];
            assert_eq!(set.dense_ids[dense_index], id);
            assert_eq!(set.get(id), Some(&(id as i32 * 10)));
        }
    }
}
#[cfg(test)]
mod entity_manager_tests {
    use crate::{
        asset_manager_new::{asset_manager_new::AssetManagerNew, gltf::GltfAsset},
        world::{
            components::{
                MeshAcessor, MeshCollectionComponent, MeshCollectionDescriptor, RigidAnimationMode,
            },
            entity_manager::EntityManager,
        },
    };

    #[test]
    fn setup_and_create() {
        let mut manager = EntityManager::new();
        let entity = manager.new_entity().unwrap();
        assert!(entity.0 == 0);
        let entity2 = manager.new_entity().unwrap();
        assert!(entity2.0 == 1);
    }

    #[test]
    fn add_components() {
        let mut asset_manager = AssetManagerNew::new();
        let box_asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();
        let mut manager = EntityManager::new();
        let entity = manager.new_entity().unwrap();
        let mesh = MeshCollectionComponent::new(MeshCollectionDescriptor {
            // MeshCollection
            resource_backing: box_asset,
            allocation_handle: None,
            mesh_accessor: MeshAcessor::All,
            rigid_animation_mode: RigidAnimationMode::Shared,
        });
        manager.add_mesh_collection_for_entity(entity, mesh);

        let _ = manager.mesh_collections.get(entity.0 as usize).unwrap();

        unsafe {
            manager.mesh_collections.dense[0].assume_init_ref();
        }
    }
}
