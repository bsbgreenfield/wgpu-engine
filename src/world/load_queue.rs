use std::collections::{HashMap, HashSet};

use crate::{
    asset_manager_new::{AssetHandle, AssetLoadResult, asset_manager_new::AssetManagerNew},
    world::{
        WorldUpdateError,
        entity_manager::{EntityHandle, EntityManager},
        scene::{Scene, SceneId, SceneLoadLevel},
    },
};

pub(super) struct SceneLoadJob {
    load_level: SceneLoadLevel,
    entity_load_jobs: Vec<EntityHandle>,
}

struct EntityLoadJob {
    ref_count: usize,
    asset_load_jobs: Vec<AssetHandle>,
}

struct AssetLoadJob {
    current_load_level: SceneLoadLevel,
    max_load_level: SceneLoadLevel,
    ref_count: usize,
}
pub(super) struct EntityLoadQueue {
    pub(super) completed_queue: HashMap<SceneId, SceneLoadJob>,
    pub(super) pending_asset_uploads: Vec<AssetHandle>,
    scene_jobs: HashMap<SceneId, SceneLoadJob>,
    entity_jobs: HashMap<EntityHandle, EntityLoadJob>,
    asset_jobs: HashMap<AssetHandle, AssetLoadJob>,
}

impl EntityLoadQueue {
    pub(super) fn new() -> Self {
        Self {
            scene_jobs: HashMap::new(),
            pending_asset_uploads: Vec::new(),
            completed_queue: HashMap::new(),
            entity_jobs: HashMap::new(),
            asset_jobs: HashMap::new(),
        }
    }

    pub(super) fn has_pending_scene_job(&self, scene_id: SceneId) -> bool {
        self.scene_jobs.get(&scene_id).is_some()
    }

    pub(super) fn dequeue_spawned_scene(&mut self, scene_id: SceneId) {
        let Some(scene_job) = self.completed_queue.remove(&scene_id) else {
            return;
        };

        for entity in scene_job.entity_load_jobs {
            let entity_ref_count = {
                let job = self.entity_jobs.get_mut(&entity).unwrap();
                job.ref_count -= 1;
                job.ref_count
            };

            if entity_ref_count == 0 {
                let entity_job = self.entity_jobs.remove(&entity).unwrap();
                for asset in entity_job.asset_load_jobs {
                    let asset_job = self.asset_jobs.get_mut(&asset).unwrap();
                    asset_job.ref_count -= 1;
                    if asset_job.ref_count == 0 {
                        self.asset_jobs.remove(&asset);
                    }
                }
            }
        }
    }

    pub(super) fn new_scene_job(
        &mut self,
        scene: &Scene,
        entity_manager: &EntityManager,
    ) -> Result<(), WorldUpdateError> {
        let entities = scene.entitites.clone();
        for entity in entities.iter() {
            self.new_entity_load(
                entity.clone(),
                &entity_manager.rbcs_of(*entity),
                scene.load_level,
            )?;
        }
        self.scene_jobs.insert(
            scene.scene_id,
            SceneLoadJob {
                load_level: scene.load_level,
                entity_load_jobs: entities,
            },
        );

        Ok(())
    }

    fn new_entity_load(
        &mut self,
        entity: EntityHandle,
        assets: &HashSet<AssetHandle>,
        load_level: SceneLoadLevel,
    ) -> Result<&EntityLoadJob, WorldUpdateError> {
        if let Some(existing) = self.entity_jobs.get_mut(&entity) {
            existing.ref_count += 1;
            return Ok(self.entity_jobs.get(&entity).unwrap());
        }
        let s = self.entity_jobs.insert(
            entity,
            EntityLoadJob {
                ref_count: 1,
                asset_load_jobs: assets.iter().map(|a| a.to_owned()).collect(),
            },
        );

        if s.is_some() {
            return Err(WorldUpdateError::EntityLoadAlreadyEnqeued(entity));
        }

        for asset in assets {
            if let Some(asset_job) = self.asset_jobs.get_mut(asset) {
                if load_level > asset_job.max_load_level {
                    asset_job.max_load_level = load_level;
                }
                asset_job.ref_count += 1;
            } else {
                self.asset_jobs.insert(
                    *asset,
                    AssetLoadJob {
                        ref_count: 1,
                        max_load_level: load_level,
                        current_load_level: SceneLoadLevel::NotLoaded,
                    },
                );
            }
        }
        Ok(self.entity_jobs.get(&entity).as_ref().unwrap())
    }

    pub(super) fn poll_scene_job(
        &mut self,
        scene_id: SceneId,
        manager: &mut AssetManagerNew,
    ) -> Result<(), WorldUpdateError> {
        let mut complete = true;
        let entity_count = self.scene_jobs[&scene_id].entity_load_jobs.len();
        for i in 0..entity_count {
            let load_level = self.scene_jobs[&scene_id].load_level;
            let entity = self.scene_jobs[&scene_id].entity_load_jobs[i];
            if !self.poll_entity(entity, manager, load_level)? {
                complete = false;
            }
        }
        if complete {
            let completed_scene_job = self.scene_jobs.remove_entry(&scene_id).unwrap();
            self.completed_queue
                .insert(completed_scene_job.0, completed_scene_job.1);
        }
        Ok(())
    }

    // pass through fn for now, but in the future I may want finer grain control over per entity loads
    fn poll_entity(
        &mut self,
        entity_handle: EntityHandle,
        manager: &mut AssetManagerNew,
        load_level: SceneLoadLevel,
    ) -> Result<bool, WorldUpdateError> {
        if self.poll_assets_for_job(entity_handle, manager, load_level)? {
            return Ok(true);
        } else {
            return Ok(false);
        }
    }

    fn poll_assets_for_job(
        &mut self,
        entity: EntityHandle,
        asset_manager: &mut AssetManagerNew,
        load_level: SceneLoadLevel,
    ) -> Result<bool, WorldUpdateError> {
        let job = self.entity_jobs.get(&entity).unwrap();
        let mut counter = job.asset_load_jobs.len();
        for asset in job.asset_load_jobs.iter() {
            let asset_job = self.asset_jobs.get_mut(asset).unwrap();
            if asset_job.current_load_level >= asset_job.max_load_level {
                counter -= 1;
                continue;
            } else {
                let load_result = asset_manager
                    .set_minumum_load_level(asset, asset_job.max_load_level)
                    .unwrap();
                if SceneLoadLevel::from(&load_result) >= load_level {
                    counter -= 1;
                } else {
                    match load_result {
                        AssetLoadResult::PendingGPU => {
                            self.pending_asset_uploads.push(*asset);
                        }
                        _ => {}
                    }
                }
                asset_job.current_load_level = SceneLoadLevel::from(&load_result);
            }
        }
        Ok(counter == 0)
    }
}
#[cfg(test)]
mod load_queue_tests {
    use crate::{
        app::renderer::GPUAllocationHandle,
        asset_manager_new::{asset_manager_new::AssetManagerNew, gltf::GltfAsset},
        world::{
            components::{
                MeshAcessor, MeshCollectionComponent, MeshCollectionDescriptor, RigidAnimationMode,
            },
            entity_manager::EntityManager,
            load_queue::EntityLoadQueue,
            scene::{Scene, SceneLoadLevel},
            world::WorldUpdateDelta,
        },
    };

    fn make_box_scene(
        id: usize,
        asset_manager: &mut AssetManagerNew,
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
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
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
        let mut asset_manager = AssetManagerNew::new();
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
        let mut asset_manager = AssetManagerNew::new();
        let mut entity_manager = EntityManager::new();

        let scene = make_box_scene(
            1,
            &mut asset_manager,
            &mut entity_manager,
            SceneLoadLevel::CPU,
        );
        let scene_id = scene.scene_id;

        queue.new_scene_job(&scene, &entity_manager).unwrap();

        queue.poll_scene_job(scene_id, &mut asset_manager).unwrap();

        assert!(!queue.has_pending_scene_job(scene_id));
        assert!(queue.completed_queue.contains_key(&scene_id));
    }
    #[test]
    fn dequeue_cleans_up_all_jobs() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManagerNew::new();
        let mut entity_manager = EntityManager::new();

        let scene = make_box_scene(
            1,
            &mut asset_manager,
            &mut entity_manager,
            SceneLoadLevel::CPU,
        );
        let scene_id = scene.scene_id;

        queue.new_scene_job(&scene, &entity_manager).unwrap();

        queue.poll_scene_job(scene_id, &mut asset_manager).unwrap();

        queue.dequeue_spawned_scene(scene_id);

        assert!(queue.entity_jobs.is_empty());
        assert!(queue.asset_jobs.is_empty());
        assert!(queue.completed_queue.is_empty());
    }
    #[test]
    fn shared_entity_ref_counting_across_two_scenes() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManagerNew::new();
        let mut entity_manager = EntityManager::new();

        // One asset, one entity shared by both scenes.
        let asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();
        let shared_entity = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            shared_entity,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: asset,
                allocation_handle: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
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

        queue.poll_scene_job(id_a, &mut asset_manager).unwrap();
        queue.poll_scene_job(id_b, &mut asset_manager).unwrap();

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
        let mut asset_manager = AssetManagerNew::new();
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

        queue.poll_scene_job(id_a, &mut asset_manager).unwrap();
        queue.poll_scene_job(id_b, &mut asset_manager).unwrap();

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
        let mut asset_manager = AssetManagerNew::new();
        let mut entity_manager = EntityManager::new();

        let shared_asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();

        let entity_a = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_a,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
            }),
        );

        let entity_b = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_b,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
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

        queue.poll_scene_job(id_a, &mut asset_manager).unwrap();
        queue.poll_scene_job(id_b, &mut asset_manager).unwrap();

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
        let mut asset_manager = AssetManagerNew::new();
        let mut entity_manager = EntityManager::new();

        let shared_asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();

        let entity_a = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_a,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
            }),
        );
        let entity_b = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_b,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
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

        queue.poll_scene_job(id_a, &mut asset_manager).unwrap();

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
            .poll_scene_job(id_b, &mut asset_manager)
            .expect("scene b should still be active");

        // the assets of the scene have only reached CPU, so this job should stil be active
        assert!(queue.has_pending_scene_job(id_b));
        assert!(!queue.completed_queue.contains_key(&id_b));

        // Renderer should have been told to upload the asset.
        let asset_did_load_count = queue.pending_asset_uploads.len();
        assert_eq!(asset_did_load_count, 1);

        asset_manager
            .register_asset_gpu_residency(&shared_asset, GPUAllocationHandle::mock(0))
            .expect("should registered with the asset manager");

        queue
            .poll_scene_job(id_b, &mut asset_manager)
            .expect("scene b should still be active");
        assert!(!queue.has_pending_scene_job(id_b));
        assert!(queue.completed_queue.contains_key(&id_b));
    }

    #[test]
    fn shared_asset_both_cpu_second_scene_resolves_from_cache() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManagerNew::new();
        let mut entity_manager = EntityManager::new();

        let shared_asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();

        let entity_a = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_a,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
            }),
        );
        let entity_b = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_b,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
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

        queue.poll_scene_job(id_a, &mut asset_manager).unwrap();

        assert!(!queue.has_pending_scene_job(id_a));
        assert_eq!(
            queue.asset_jobs[&shared_asset].current_load_level,
            SceneLoadLevel::CPU
        );

        // Scene B polls — asset already at CPU, should resolve immediately.
        queue.poll_scene_job(id_b, &mut asset_manager).unwrap();
        assert!(!queue.has_pending_scene_job(id_b));
        assert!(queue.completed_queue.contains_key(&id_b));

        // No AssetDidLoad deltas — asset never needed GPU upload.
        assert!(queue.pending_asset_uploads.is_empty());
    }

    #[test]
    fn shared_asset_gpu_cpu_second_scene_resolves_from_cache() {
        let mut queue = EntityLoadQueue::new();
        let mut asset_manager = AssetManagerNew::new();
        let mut entity_manager = EntityManager::new();

        let shared_asset = asset_manager.register_asset::<GltfAsset>("box").unwrap();

        let entity_a = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_a,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
            }),
        );
        let entity_b = entity_manager.new_entity().unwrap();
        entity_manager.add_mesh_collection_for_entity(
            entity_b,
            MeshCollectionComponent::new(MeshCollectionDescriptor {
                resource_backing: shared_asset,
                allocation_handle: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
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

        // poll b first
        queue.poll_scene_job(id_b, &mut asset_manager).unwrap();

        queue
            .poll_scene_job(id_a, &mut asset_manager)
            .expect("should still be active ");

        assert!(!queue.has_pending_scene_job(id_a));
        assert_eq!(
            queue.asset_jobs[&shared_asset].current_load_level,
            SceneLoadLevel::CPU
        );

        assert!(queue.has_pending_scene_job(id_b));
        assert!(!queue.completed_queue.contains_key(&id_b));

        assert_eq!(queue.pending_asset_uploads.len(), 1);
        assert_eq!(queue.pending_asset_uploads[0], shared_asset);
    }
}
