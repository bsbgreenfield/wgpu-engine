#[cfg(test)]
mod load_queue_tests {
    use crate::{
        app::renderer::GPUAllocationHandle,
        asset_manager::{asset_manager::AssetManager, gltf_assets::GltfAsset},
        world::{
            components::{MeshCollectionComponent, MeshCollectionDescriptor},
            entity_manager::EntityManager,
            load_queue::EntityLoadQueue,
            scene::{Scene, SceneLoadLevel},
            world::WorldUpdateDelta,
        },
    };

    fn make_box_scene(
        id: usize,
        asset_manager: &mut AssetManager,
        entity_manager: &mut EntityManager,
        load_level: SceneLoadLevel,
    ) -> Scene {
        let asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();
        let entity = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );
        let mut scene = Scene::new_with_id(id);
        scene.load_level = load_level;
        scene.add_entity(entity);
        scene
    }
    #[test]
    fn enqueue_creates_pending_job() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let scene = make_box_scene(
            1,
            &mut asset_manager,
            &mut entity_manager,
            SceneLoadLevel::CPU,
        );
        let scene_id = scene.scene_id;

        queue.new_scene_job(&scene, &entity_manager).unwrap();

        assert!(queue.has_pending_scene_job(scene_id));
        assert!(queue.completed_queue.is_empty());
    }

    /// polling should complete the load because the asset load is synchronous
    #[test]
    fn poll_completes_cpu_scene() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let scene = make_box_scene(
            1,
            &mut asset_manager,
            &mut entity_manager,
            SceneLoadLevel::CPU,
        );
        let scene_id = scene.scene_id;

        queue.new_scene_job(&scene, &entity_manager).unwrap();

        let mut deltas: Vec<WorldUpdateDelta> = Vec::new();
        queue
            .poll_scene_job(scene_id, &mut asset_manager, &mut deltas)
            .unwrap();

        assert!(!queue.has_pending_scene_job(scene_id));
        assert!(queue.completed_queue.contains_key(&scene_id));
    }
    #[test]
    fn dequeue_cleans_up_all_jobs() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let scene = make_box_scene(
            1,
            &mut asset_manager,
            &mut entity_manager,
            SceneLoadLevel::CPU,
        );
        let scene_id = scene.scene_id;

        queue.new_scene_job(&scene, &entity_manager).unwrap();

        let mut deltas = Vec::new();
        queue
            .poll_scene_job(scene_id, &mut asset_manager, &mut deltas)
            .unwrap();

        queue.dequeue_spawned_scene(scene_id);

        assert!(queue.entity_jobs.is_empty());
        assert!(queue.asset_jobs.is_empty());
        assert!(queue.completed_queue.is_empty());
    }
    #[test]
    fn shared_entity_ref_counting_across_two_scenes() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        // One asset, one entity shared by both scenes.
        let asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();
        let shared_entity = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            shared_entity,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );

        let mut scene_a = Scene::new_with_id(1);
        scene_a.load_level = SceneLoadLevel::CPU;
        scene_a.add_entity(shared_entity);

        let mut scene_b = Scene::new_with_id(2);
        scene_b.load_level = SceneLoadLevel::CPU;
        scene_b.add_entity(shared_entity);

        let id_a = scene_a.scene_id;
        let id_b = scene_b.scene_id;

        queue.new_scene_job(&scene_a, &entity_manager).unwrap();
        queue.new_scene_job(&scene_b, &entity_manager).unwrap();

        // Entity should have ref_count == 2 at this point.
        assert_eq!(queue.entity_jobs[&shared_entity].ref_count, 2);

        let mut deltas = Vec::new();
        queue
            .poll_scene_job(id_a, &mut asset_manager, &mut deltas)
            .unwrap();
        queue
            .poll_scene_job(id_b, &mut asset_manager, &mut deltas)
            .unwrap();

        // Dequeue scene A — entity still referenced by scene B.
        queue.dequeue_spawned_scene(id_a);
        assert!(
            queue.entity_jobs.contains_key(&shared_entity),
            "entity should still be tracked"
        );
        assert_eq!(queue.entity_jobs[&shared_entity].ref_count, 1);

        // Dequeue scene B — entity and asset fully released.
        queue.dequeue_spawned_scene(id_b);
        assert!(queue.entity_jobs.is_empty());
        assert!(queue.asset_jobs.is_empty());
    }
    #[test]
    fn independent_scenes_dont_interfere() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let scene_a = make_box_scene(
            1,
            &mut asset_manager,
            &mut entity_manager,
            SceneLoadLevel::CPU,
        );
        let scene_b = make_box_scene(
            2,
            &mut asset_manager,
            &mut entity_manager,
            SceneLoadLevel::CPU,
        );

        let id_a = scene_a.scene_id;
        let id_b = scene_b.scene_id;

        queue.new_scene_job(&scene_a, &entity_manager).unwrap();
        queue.new_scene_job(&scene_b, &entity_manager).unwrap();

        let mut deltas = Vec::new();
        queue
            .poll_scene_job(id_a, &mut asset_manager, &mut deltas)
            .unwrap();
        queue
            .poll_scene_job(id_b, &mut asset_manager, &mut deltas)
            .unwrap();

        queue.dequeue_spawned_scene(id_a);

        // Scene B's entity and asset should still be present.
        assert_eq!(queue.entity_jobs.len(), 1);
        assert_eq!(queue.asset_jobs.len(), 1);
        assert!(queue.completed_queue.contains_key(&id_b));

        queue.dequeue_spawned_scene(id_b);
        assert!(queue.entity_jobs.is_empty());
        assert!(queue.asset_jobs.is_empty());
    }
    #[test]
    fn shared_asset_persists_until_last_entity_dequeued() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let shared_asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();

        let entity_a = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_a,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );

        let entity_b = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_b,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );

        let mut scene_a = Scene::new_with_id(1);
        scene_a.load_level = SceneLoadLevel::CPU;
        scene_a.add_entity(entity_a);

        let mut scene_b = Scene::new_with_id(2);
        scene_b.load_level = SceneLoadLevel::CPU;
        scene_b.add_entity(entity_b);

        let id_a = scene_a.scene_id;
        let id_b = scene_b.scene_id;

        queue.new_scene_job(&scene_a, &entity_manager).unwrap();
        queue.new_scene_job(&scene_b, &entity_manager).unwrap();

        // Asset is referenced by two distinct entities — ref_count should be 2.
        assert_eq!(queue.asset_jobs[&shared_asset].ref_count, 2);

        let mut deltas = Vec::new();
        queue
            .poll_scene_job(id_a, &mut asset_manager, &mut deltas)
            .unwrap();
        queue
            .poll_scene_job(id_b, &mut asset_manager, &mut deltas)
            .unwrap();

        // Dequeue scene A — entity_a removed, but asset still referenced by entity_b.
        queue.dequeue_spawned_scene(id_a);
        assert!(
            queue.asset_jobs.contains_key(&shared_asset),
            "asset job should persist while entity_b still holds a reference"
        );
        assert_eq!(queue.asset_jobs[&shared_asset].ref_count, 1);

        // Dequeue scene B — entity_b removed, asset ref_count hits 0, fully cleaned up.
        queue.dequeue_spawned_scene(id_b);
        assert!(queue.asset_jobs.is_empty());
        assert!(queue.entity_jobs.is_empty());
    }
    #[test]
    fn shared_asset_cpu_scene_resolves_gpu_scene_stays_pending() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let shared_asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();

        let entity_a = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_a,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );
        let entity_b = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_b,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );

        // A is CPU
        let mut scene_a = Scene::new_with_id(1);
        scene_a.load_level = SceneLoadLevel::CPU;
        scene_a.add_entity(entity_a);
        let id_a = scene_a.scene_id;

        // B is GPU
        let mut scene_b = Scene::new_with_id(2);
        scene_b.load_level = SceneLoadLevel::GPU;
        scene_b.add_entity(entity_b);
        let id_b = scene_b.scene_id;

        queue.new_scene_job(&scene_a, &entity_manager).unwrap();
        queue.new_scene_job(&scene_b, &entity_manager).unwrap();

        // Shared asset is referenced by two entities.
        assert_eq!(queue.asset_jobs[&shared_asset].ref_count, 2);

        let mut deltas = Vec::new();
        queue
            .poll_scene_job(id_a, &mut asset_manager, &mut deltas)
            .unwrap();

        // Scene A should be done; asset state advanced to CPU.
        assert!(!queue.has_pending_scene_job(id_a));
        assert!(queue.completed_queue.contains_key(&id_a));
        assert_eq!(
            queue.asset_jobs[&shared_asset].current_load_level,
            SceneLoadLevel::CPU
        );
        assert_eq!(
            queue.asset_jobs[&shared_asset].max_load_level,
            SceneLoadLevel::GPU
        );

        queue
            .poll_scene_job(id_b, &mut asset_manager, &mut deltas)
            .expect("scene b should still be active");

        // the assets of the scene have only reached CPU, so this job should stil be active
        assert!(queue.has_pending_scene_job(id_b));
        assert!(!queue.completed_queue.contains_key(&id_b));

        // Renderer should have been told to upload the asset.
        let asset_did_load_count = deltas
            .iter()
            .filter(|d| matches!(d, WorldUpdateDelta::AssetDidLoad(_)))
            .count();
        assert_eq!(asset_did_load_count, 1);

        asset_manager
            .register_asset_gpu_residency(&GPUAllocationHandle::mock(0, shared_asset))
            .expect("should registered with the asset manager");

        queue
            .poll_scene_job(id_b, &mut asset_manager, &mut deltas)
            .expect("scene b should still be active");
        assert!(!queue.has_pending_scene_job(id_b));
        assert!(queue.completed_queue.contains_key(&id_b));
    }

    #[test]
    fn shared_asset_both_cpu_second_scene_resolves_from_cache() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let shared_asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();

        let entity_a = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_a,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );
        let entity_b = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_b,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );

        let mut scene_a = Scene::new_with_id(1);
        scene_a.load_level = SceneLoadLevel::CPU;
        scene_a.add_entity(entity_a);
        let id_a = scene_a.scene_id;

        let mut scene_b = Scene::new_with_id(2);
        scene_b.load_level = SceneLoadLevel::CPU;
        scene_b.add_entity(entity_b);
        let id_b = scene_b.scene_id;

        queue.new_scene_job(&scene_a, &entity_manager).unwrap();
        queue.new_scene_job(&scene_b, &entity_manager).unwrap();

        let mut deltas = Vec::new();
        queue
            .poll_scene_job(id_a, &mut asset_manager, &mut deltas)
            .unwrap();

        assert!(!queue.has_pending_scene_job(id_a));
        assert_eq!(
            queue.asset_jobs[&shared_asset].current_load_level,
            SceneLoadLevel::CPU
        );

        // Scene B polls — asset already at CPU, should resolve immediately.
        queue
            .poll_scene_job(id_b, &mut asset_manager, &mut deltas)
            .unwrap();
        assert!(!queue.has_pending_scene_job(id_b));
        assert!(queue.completed_queue.contains_key(&id_b));

        // No AssetDidLoad deltas — asset never needed GPU upload.
        assert!(deltas.is_empty());
    }

    #[test]
    fn shared_asset_gpu_cpu_second_scene_resolves_from_cache() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let shared_asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();

        let entity_a = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_a,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );
        let entity_b = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_b,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_ids: &[0],
            }),
        );

        let mut scene_a = Scene::new_with_id(1);
        scene_a.load_level = SceneLoadLevel::CPU;
        scene_a.add_entity(entity_a);
        let id_a = scene_a.scene_id;

        let mut scene_b = Scene::new_with_id(2);
        scene_b.load_level = SceneLoadLevel::GPU;
        scene_b.add_entity(entity_b);
        let id_b = scene_b.scene_id;

        queue.new_scene_job(&scene_a, &entity_manager).unwrap();
        queue.new_scene_job(&scene_b, &entity_manager).unwrap();

        let mut deltas = Vec::new();

        // poll b first
        queue
            .poll_scene_job(id_b, &mut asset_manager, &mut deltas)
            .unwrap();

        queue
            .poll_scene_job(id_a, &mut asset_manager, &mut deltas)
            .expect("should still be active ");

        assert!(!queue.has_pending_scene_job(id_a));
        assert_eq!(
            queue.asset_jobs[&shared_asset].current_load_level,
            SceneLoadLevel::CPU
        );

        assert!(queue.has_pending_scene_job(id_b));
        assert!(!queue.completed_queue.contains_key(&id_b));

        assert_eq!(deltas.len(), 1);
        match deltas[0] {
            WorldUpdateDelta::AssetDidLoad(handle) => {
                assert_eq!(handle, shared_asset);
            }
            _ => panic!("this shouldnt be here"),
        }
    }
}
