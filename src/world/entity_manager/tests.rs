#[cfg(test)]
mod sparse_set_tests {
    use crate::world::entity_manager::entity_manager::SparseSet;

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
    use crate::world::entity_manager::entity_manager::EntityManager;

    #[test]
    fn setup_and_create() {
        let mut manager = EntityManager::new();
        let entity = manager.new_entity().unwrap();
        assert!(entity.0 == 0);
        let entity2 = manager.new_entity().unwrap();
        assert!(entity2.0 == 1);
    }

    //  #[test]
    //  fn add_components() {
    //      let mut asset_manager = AssetManagerNew::new();
    //      let box_asset = asset_manager
    //          .register_asset::<LoadedGltfAsset>("box")
    //          .unwrap();
    //      let mut manager = EntityManager::new();
    //      let entity = manager.new_entity().unwrap();
    //      let mesh = MeshCollectionComponent::new(MeshCollectionDescriptor {
    //          // MeshCollection
    //          resource_backing: box_asset,
    //          allocation_handle: None,
    //          mesh_accessor: MeshAcessor::All,
    //          rigid_animation_mode: RigidAnimationMode::Shared,
    //      });
    //      manager.add_mesh_collection_for_entity(&entity, mesh);

    //      let _ = manager.mesh_collections.get(entity.0 as usize).unwrap();

    //      unsafe {
    //          manager.mesh_collections.dense[0].assume_init_ref();
    //      }
    //  }
}
