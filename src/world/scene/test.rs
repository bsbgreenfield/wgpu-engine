#[cfg(test)]
mod scene_tests {
    use std::collections::HashMap;

    use crate::{
        asset_manager::{
            AssetHandle, AssetResidency, ProvidesMeshData, asset_manager::AssetManager,
        },
        common::{entity::EntityHandle, instance::InstanceHandle},
        renderer::{GPUAllocationHandle, GPUInstanceHandle, PrototypeHandle},
        world::{
            entity_manager::{
                components::{MeshAcessor, MeshCollectionDescriptor, ResourceBacking},
                entity_manager::EntityManager,
            },
            instance_manager::archetypes::{APosition, Archetype, ArchetypeId},
            load_queue::AssetTransition,
            scene::{
                Scene, SceneId, SceneLoadLevel,
                builder::SceneBuilder,
                dependency_graph::DependencyGraph,
                manager::SceneManager,
                scene::{SceneDesc, SceneRuntime, Spawn},
            },
        },
    };

    /// Assets and entities, mocked so nothing touches the disk or a GPU. Every
    /// entity carries exactly one asset, so an entity is just a named asset.
    struct Fixture {
        assets: AssetManager,
        entities: EntityManager,
        next_scene: usize,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                assets: AssetManager::new(),
                entities: EntityManager::new(),
                next_scene: 0,
            }
        }

        fn unloaded(&mut self) -> AssetHandle {
            self.assets.mock_asset(AssetResidency::Registered)
        }

        fn cpu(&mut self) -> AssetHandle {
            self.assets.mock_asset(AssetResidency::CPU(0))
        }

        fn gpu(&mut self) -> AssetHandle {
            self.assets
                .mock_asset(AssetResidency::GPU(GPUAllocationHandle::mock(0), 0))
        }

        fn entity(&mut self, asset: AssetHandle) -> EntityHandle {
            let entity = self.entities.new_entity().expect("entity budget");
            self.entities.add_mesh_collection_for_entity(
                &entity,
                MeshCollectionDescriptor {
                    resource_backing: ResourceBacking::<dyn ProvidesMeshData>::new(asset),
                    mesh_accessor: MeshAcessor::All,
                    animation: None,
                },
            );
            entity
        }

        /// Build a bare `Scene` (one entity per asset) without going through a
        /// `SceneManager`, for tests that drive the graph directly.
        fn raw_scene(&mut self, assets: &[AssetHandle], children: &[SceneId]) -> Scene {
            let entities = assets.iter().map(|a| self.entity(*a)).collect();
            let id = SceneId(self.next_scene);
            self.next_scene += 1;
            Scene {
                id,
                desc: SceneDesc {
                    children: children.to_vec(),
                    entities,
                },
                runtime: SceneRuntime::default(),
            }
        }

        /// Register a scene with `manager`, one entity per asset.
        fn scene(&mut self, manager: &mut SceneManager, assets: &[AssetHandle]) -> SceneId {
            let mut builder = SceneBuilder::new();
            for asset in assets {
                let entity = self.entity(*asset);
                builder = builder.add_entity(entity);
            }
            manager
                .add_scene(builder, &self.entities)
                .expect("scene registers")
        }

        /// Drive an asset forward the way the load queue would: flip its
        /// residency, then tell the manager about the transition.
        fn promote(
            &mut self,
            manager: &mut SceneManager,
            asset: AssetHandle,
            old: SceneLoadLevel,
            new: SceneLoadLevel,
        ) {
            let residency = match new {
                SceneLoadLevel::PendingCPU => AssetResidency::PendingCPU,
                SceneLoadLevel::PendingGPU => AssetResidency::PendingGPU(0),
                SceneLoadLevel::NotLoaded => AssetResidency::Registered,
                SceneLoadLevel::CPU => AssetResidency::CPU(0),
                SceneLoadLevel::GPU => AssetResidency::GPU(GPUAllocationHandle::mock(0), 0),
            };
            self.assets.set_mock_residency(&asset, residency);
            manager.on_asset_level_changed(AssetTransition {
                handle: asset,
                old,
                new,
            });
        }
    }

    /// `asset_requests` drains into an unordered Vec; a map is what assertions want.
    fn requests(manager: &mut SceneManager) -> HashMap<AssetHandle, SceneLoadLevel> {
        manager.asset_requests().into_iter().collect()
    }

    fn state(manager: &SceneManager, scene: SceneId) -> SceneLoadLevel {
        manager.get_scene(scene.0).runtime.current_state
    }

    /// How many spawns `process_scene_events` has handed off for `scene`.
    fn queued_spawns(manager: &SceneManager, scene: SceneId) -> usize {
        manager
            .spawn_queue
            .get(&scene)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    fn spawn_at(entity: EntityHandle, x: f32) -> Spawn<dyn Archetype> {
        (
            entity,
            Box::new(APosition {
                position: cgmath::Matrix4::<f32>::from_translation(cgmath::Vector3::new(x, 0., 0.))
                    .into(),
            }),
        )
            .into()
    }

    // ---------------------------------------------------------------- graph

    #[test]
    fn add_scene_records_deduplicated_assets_and_children() {
        let mut fixture = Fixture::new();
        let mut graph = DependencyGraph::default();

        let shared = fixture.unloaded();
        let other = fixture.unloaded();
        // three entities, but only two distinct assets between them
        let scene = fixture.raw_scene(&[shared, shared, other], &[SceneId(1), SceneId(2)]);
        graph.add_scene(&scene, &fixture.entities).expect("added");

        let assets = graph.required_assets_of(SceneId(0));
        assert_eq!(
            assets.len(),
            2,
            "two entities backed by the same asset must collapse to one dependency"
        );
        assert!(assets.contains(&shared) && assets.contains(&other));

        assert_eq!(graph.children_of(SceneId(0)), &[SceneId(1), SceneId(2)]);
        assert!(
            graph.children_of(SceneId(7)).is_empty(),
            "an unknown scene has no children rather than panicking"
        );
    }

    #[test]
    fn holders_of_finds_every_scene_that_depends_on_an_asset() {
        let mut fixture = Fixture::new();
        let mut graph = DependencyGraph::default();

        let shared = fixture.unloaded();
        let solo = fixture.unloaded();
        let unheld = fixture.unloaded();

        let first = fixture.raw_scene(&[shared], &[]);
        let second = fixture.raw_scene(&[shared, solo], &[]);
        graph.add_scene(&first, &fixture.entities).expect("added");
        graph.add_scene(&second, &fixture.entities).expect("added");

        let mut holders: Vec<usize> = graph.holders_of(&shared).into_iter().map(|s| s.0).collect();
        holders.sort();
        assert_eq!(holders, vec![0, 1]);

        let solo_holders: Vec<usize> = graph.holders_of(&solo).into_iter().map(|s| s.0).collect();
        assert_eq!(solo_holders, vec![1]);

        assert_eq!(graph.holders_of(&unheld).len(), 0);
    }

    // -------------------------------------------------------------- manager

    #[test]
    fn scenes_get_sequential_ids_and_start_unloaded() {
        let mut fixture = Fixture::new();
        let mut manager = SceneManager::new();

        let asset = fixture.unloaded();
        let first = fixture.scene(&mut manager, &[asset]);
        let second = fixture.scene(&mut manager, &[asset]);

        assert_eq!(first, SceneId(0));
        assert_eq!(second, SceneId(1));
        assert_eq!(state(&manager, first), SceneLoadLevel::NotLoaded);
        assert_eq!(
            manager.get_scene(first.0).runtime.requested_level,
            SceneLoadLevel::NotLoaded
        );

        // re-requesting the level a scene already sits at is a no-op
        manager
            .set_load_level(first, SceneLoadLevel::NotLoaded, &fixture.assets)
            .expect("no-op set");
        assert!(requests(&mut manager).is_empty());
    }

    #[test]
    fn raising_requests_only_the_assets_that_are_not_resident_yet() {
        let mut fixture = Fixture::new();
        let mut manager = SceneManager::new();

        let cold = fixture.unloaded();
        let warm = fixture.cpu();
        let scene = fixture.scene(&mut manager, &[cold, warm]);

        manager
            .set_load_level(scene, SceneLoadLevel::CPU, &fixture.assets)
            .expect("raise to cpu");

        let requested = requests(&mut manager);
        assert_eq!(
            requested.len(),
            1,
            "the already-CPU-resident asset must not be requested again"
        );
        assert_eq!(requested[&cold], SceneLoadLevel::CPU);
        assert_eq!(
            state(&manager, scene),
            SceneLoadLevel::NotLoaded,
            "the scene stays unloaded while one asset is still outstanding"
        );
    }

    #[test]
    fn raising_to_an_already_resident_level_loads_the_scene_immediately() {
        let mut fixture = Fixture::new();
        let mut manager = SceneManager::new();

        let asset = fixture.gpu();
        let scene = fixture.scene(&mut manager, &[asset]);
        let entity = EntityHandle(0);

        // queued before the scene is loaded — must wait for the load
        manager
            .add_instances(scene, vec![spawn_at(entity, 0.), spawn_at(entity, 1.)])
            .expect("queue spawns");
        manager.process_scene_events().expect("process");
        assert_eq!(queued_spawns(&manager, scene), 0);

        manager
            .set_load_level(scene, SceneLoadLevel::GPU, &fixture.assets)
            .expect("raise to gpu");

        assert!(
            requests(&mut manager).is_empty(),
            "nothing to load when every asset is already GPU resident"
        );
        assert_eq!(state(&manager, scene), SceneLoadLevel::GPU);

        manager.process_scene_events().expect("process");
        assert_eq!(queued_spawns(&manager, scene), 2);
        assert!(
            manager.get_scene(scene.0).runtime.spawn_queue.is_empty(),
            "the scene's own queue is emptied when it hands off to the manager"
        );
    }

    #[test]
    fn lowering_keeps_assets_alive_for_other_holders_and_despawns_instances() {
        let mut fixture = Fixture::new();
        let mut manager = SceneManager::new();

        let shared = fixture.gpu();
        let first = fixture.scene(&mut manager, &[shared]);
        let second = fixture.scene(&mut manager, &[shared]);

        for scene in [first, second] {
            manager
                .set_load_level(scene, SceneLoadLevel::GPU, &fixture.assets)
                .expect("raise to gpu");
        }
        let _ = requests(&mut manager);

        let instances = vec![
            InstanceHandle::mock(ArchetypeId::Position, EntityHandle(0), 0, 0),
            InstanceHandle::mock(ArchetypeId::Position, EntityHandle(0), 1, 0),
        ];
        manager
            .add_instance_handles(first, instances.clone())
            .expect("register instances");

        manager
            .set_load_level(first, SceneLoadLevel::NotLoaded, &fixture.assets)
            .expect("drop first");

        assert!(
            requests(&mut manager).is_empty(),
            "the shared asset must stay resident while the second scene still wants it"
        );
        assert_eq!(state(&manager, first), SceneLoadLevel::NotLoaded);
        assert_eq!(manager.despawn_queue, instances);

        assert!(manager.instances_of(first).is_empty());

        manager
            .set_load_level(second, SceneLoadLevel::NotLoaded, &fixture.assets)
            .expect("drop second");

        let gpu_handles: Vec<GPUInstanceHandle> = instances
            .iter()
            .enumerate()
            .map(|(i, instance)| {
                let gpu_handle = GPUInstanceHandle {
                    prototype: PrototypeHandle::new(0),
                    instance_id: i as u32,
                };
                manager
                    .inflight_despawns
                    .insert(gpu_handle, instance.clone());
                gpu_handle
            })
            .collect();
        manager.ack_despawns(gpu_handles);

        let requested = requests(&mut manager);
        assert_eq!(
            requested[&shared],
            SceneLoadLevel::NotLoaded,
            "with the last holder gone the asset is finally released"
        );
    }

    // ---------------------------------------------------------------- loading

    #[test]
    fn a_scene_loads_only_once_every_asset_has_arrived() {
        let mut fixture = Fixture::new();
        let mut manager = SceneManager::new();

        let first = fixture.unloaded();
        let second = fixture.unloaded();
        let scene = fixture.scene(&mut manager, &[first, second]);

        manager
            .set_load_level(scene, SceneLoadLevel::GPU, &fixture.assets)
            .expect("raise to gpu");
        assert_eq!(requests(&mut manager).len(), 2);

        fixture.promote(
            &mut manager,
            first,
            SceneLoadLevel::NotLoaded,
            SceneLoadLevel::GPU,
        );
        assert_eq!(
            state(&manager, scene),
            SceneLoadLevel::NotLoaded,
            "one of two assets is not enough"
        );

        fixture.promote(
            &mut manager,
            second,
            SceneLoadLevel::NotLoaded,
            SceneLoadLevel::GPU,
        );
        assert_eq!(state(&manager, scene), SceneLoadLevel::GPU);
    }

    #[test]
    fn spawns_queued_before_the_load_flush_when_the_scene_reaches_gpu() {
        let mut fixture = Fixture::new();
        let mut manager = SceneManager::new();

        let asset = fixture.unloaded();
        let scene = fixture.scene(&mut manager, &[asset]);

        manager
            .add_instances(scene, vec![spawn_at(EntityHandle(0), 0.)])
            .expect("queue spawn");
        manager
            .set_load_level(scene, SceneLoadLevel::GPU, &fixture.assets)
            .expect("raise to gpu");
        manager.process_scene_events().expect("process");
        assert_eq!(queued_spawns(&manager, scene), 0);

        // reaching CPU is progress, but not what the scene asked for
        fixture.promote(
            &mut manager,
            asset,
            SceneLoadLevel::NotLoaded,
            SceneLoadLevel::CPU,
        );
        manager.process_scene_events().expect("process");
        assert_eq!(state(&manager, scene), SceneLoadLevel::NotLoaded);
        assert_eq!(queued_spawns(&manager, scene), 0);

        fixture.promote(
            &mut manager,
            asset,
            SceneLoadLevel::CPU,
            SceneLoadLevel::GPU,
        );
        manager.process_scene_events().expect("process");
        assert_eq!(state(&manager, scene), SceneLoadLevel::GPU);
        assert_eq!(queued_spawns(&manager, scene), 1);
    }

    #[test]
    fn instances_added_after_the_load_spawn_on_the_next_pass() {
        let mut fixture = Fixture::new();
        let mut manager = SceneManager::new();

        let asset = fixture.gpu();
        let scene = fixture.scene(&mut manager, &[asset]);
        manager
            .set_load_level(scene, SceneLoadLevel::GPU, &fixture.assets)
            .expect("raise to gpu");
        manager.process_scene_events().expect("process");
        assert_eq!(queued_spawns(&manager, scene), 0);

        manager
            .add_instances(
                scene,
                vec![spawn_at(EntityHandle(0), 1.), spawn_at(EntityHandle(0), 2.)],
            )
            .expect("queue spawns");
        manager.process_scene_events().expect("process");
        assert_eq!(queued_spawns(&manager, scene), 2);

        // the world drains the queue each frame; a later batch starts from empty
        manager.spawn_queue.clear();
        manager
            .add_instances(scene, vec![spawn_at(EntityHandle(0), 3.)])
            .expect("queue spawn");
        manager.process_scene_events().expect("process");
        assert_eq!(queued_spawns(&manager, scene), 1);
    }

    #[test]
    fn an_asset_transition_only_unblocks_the_holders_that_were_waiting_on_it() {
        let mut fixture = Fixture::new();
        let mut manager = SceneManager::new();

        let shared = fixture.unloaded();
        let modest = fixture.scene(&mut manager, &[shared]);
        let greedy = fixture.scene(&mut manager, &[shared]);

        manager
            .set_load_level(modest, SceneLoadLevel::CPU, &fixture.assets)
            .expect("raise to cpu");
        manager
            .set_load_level(greedy, SceneLoadLevel::GPU, &fixture.assets)
            .expect("raise to gpu");

        fixture.promote(
            &mut manager,
            shared,
            SceneLoadLevel::NotLoaded,
            SceneLoadLevel::CPU,
        );
        assert_eq!(state(&manager, modest), SceneLoadLevel::CPU);
        assert_eq!(
            state(&manager, greedy),
            SceneLoadLevel::NotLoaded,
            "a CPU arrival does not satisfy a GPU request"
        );

        fixture.promote(
            &mut manager,
            shared,
            SceneLoadLevel::CPU,
            SceneLoadLevel::GPU,
        );
        assert_eq!(state(&manager, greedy), SceneLoadLevel::GPU);
        assert_eq!(
            state(&manager, modest),
            SceneLoadLevel::CPU,
            "the CPU holder was already satisfied and must not be counted down twice"
        );
    }
}
