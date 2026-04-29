#[cfg(test)]
mod integration_tests {

    use cgmath::SquareMatrix;

    use crate::{
        app::{
            app::App,
            app_config::AppConfig,
            app_state::AppState,
            renderer::{
                DrawItem, DrawPacket, Instruction, Operations, RenderUpdateDelta, VMValue,
                renderer::Renderer,
            },
        },
        asset_manager_new::asset_manager_new::AssetManagerNew,
        world::{
            self,
            entity_manager::{EntityHandle, EntityManager},
            instance_manager::{self, APosition, ArchetypeId, InstanceHandle},
            scene::{Scene, SceneLoadLevel},
            world::{World, WorldUpdateDelta},
        },
    };

    enum TestCases {
        Box,
        Fox,
        BoxFox,
    }

    /// Variant-only mirrors of WorldUpdateDelta — use these to declare what a frame should produce.
    #[derive(Debug)]
    enum WorldDeltaKind {
        AssetDidLoad,
        EntityDidSpawn,
        EntityDidLoad,
    }

    /// Variant-only mirrors of RenderUpdateDelta — use these to declare what the renderer should emit.
    #[derive(Debug)]
    enum RenderDeltaKind {
        AssetGPULoaded,
        EntitySpawn,
    }

    fn get_bytecode<'a>(
        world: &'a World,
        deltas: &'a [WorldUpdateDelta],
    ) -> (Vec<VMValue<'a>>, Vec<Instruction>) {
        let mut constants = Vec::<VMValue<'a>>::new();
        let mut instructions = Vec::<Instruction>::new();

        for delta in deltas.iter() {
            delta.gen_bytecode(world, &mut constants, &mut instructions);
        }

        (constants, instructions)
    }

    fn assert_world_deltas(actual: &[WorldUpdateDelta], expected: &[WorldDeltaKind]) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "world delta count mismatch actual {:?} expected: {:?}",
            actual,
            expected
        );
        for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            let matches = matches!(
                (a, e),
                (
                    WorldUpdateDelta::AssetDidLoad(_),
                    WorldDeltaKind::AssetDidLoad
                ) | (
                    WorldUpdateDelta::EntityDidSpawn(_),
                    WorldDeltaKind::EntityDidSpawn
                ) | (
                    WorldUpdateDelta::EntityDidLoad(_),
                    WorldDeltaKind::EntityDidLoad
                )
            );
            assert!(matches, "world delta[{i}] variant mismatch");
        }
    }

    fn assert_render_deltas(actual: &[RenderUpdateDelta], expected: &[RenderDeltaKind]) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "render delta count mismatch. actual: {:?}, expected: {:?}",
            actual,
            expected
        );
        for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            let matches = matches!(
                (a, e),
                (
                    RenderUpdateDelta::AssetGPULoaded(..),
                    RenderDeltaKind::AssetGPULoaded
                ) | (
                    RenderUpdateDelta::EntitySpawned(_),
                    RenderDeltaKind::EntitySpawn
                )
            );
            assert!(matches, "render delta[{i}] variant mismatch");
        }
    }

    async fn setup_world<'a>(test_case: TestCases) -> App<'a> {
        let mut app = App::new();
        let config = AppConfig::new_headless().await;
        let renderer = Renderer::new(&config);
        let entity_manager = EntityManager::new();
        let mut world = World::new(1.0, entity_manager, &config.device).unwrap();
        let scene = match test_case {
            TestCases::Box => Scene::box_scene(&mut world).expect("box init"),
            TestCases::Fox => Scene::fox_scene(&mut world).expect("fox init"),
            TestCases::BoxFox => Scene::fox_box(&mut world).expect("fox box init"),
        };
        world.add_scene(scene);
        app.world = Some(world);
        app.app_config = Some(config);
        app.renderer = Some(renderer);
        app.app_state = AppState {};
        app.surface_ready = true;

        app
    }

    fn run_frame(
        app: &mut App<'_>,
        expected_world_deltas: &[WorldDeltaKind],
        expected_render_deltas: &[RenderDeltaKind],
    ) {
        let deltas = app.update_world().unwrap_or_else(|e| panic!("{}", e));
        assert_world_deltas(&deltas, expected_world_deltas);

        let (constants, instructions) = get_bytecode(app.world.as_ref().unwrap(), &deltas);

        let render_deltas = app
            .renderer
            .as_mut()
            .unwrap()
            .update(
                constants,
                instructions,
                &app.app_config.as_ref().unwrap().queue,
            )
            .unwrap_or_else(|e| panic!("{}", e));

        assert_render_deltas(&render_deltas, expected_render_deltas);
        app.world.as_mut().unwrap().post_frame_update(render_deltas);
    }

    fn run_frame_unchecked(app: &mut App<'_>) {
        let deltas = app.update_world().unwrap_or_else(|e| panic!("{}", e));
        let (constants, instructions) = get_bytecode(app.world.as_ref().unwrap(), &deltas);

        let render_deltas = app
            .renderer
            .as_mut()
            .unwrap()
            .update(
                constants,
                instructions,
                &app.app_config.as_ref().unwrap().queue,
            )
            .unwrap_or_else(|e| panic!("{}", e));
        app.world.as_mut().unwrap().post_frame_update(render_deltas);
    }

    fn gen_draw_calls(app: &mut App) {
        app.draw_packet.clear();
        app.world
            .as_ref()
            .unwrap()
            .instance_manager
            .gen_draw_calls(&mut app.draw_packet);
    }

    #[test]
    fn render_box() {
        pollster::block_on(async {
            let mut app = setup_world(TestCases::Box).await;

            run_frame(
                &mut app,
                &[WorldDeltaKind::AssetDidLoad], // expected world update deltas
                &[RenderDeltaKind::AssetGPULoaded], // expected render deltas
            );
            gen_draw_calls(&mut app);
            assert!(app.draw_packet.is_empty());

            run_frame(
                &mut app,
                &[WorldDeltaKind::EntityDidSpawn],
                &[RenderDeltaKind::EntitySpawn],
            );
            let instance_manager = &app.world.as_ref().unwrap().instance_manager;
            assert_eq!(instance_manager.get_all_instances().len(), 1);
            assert_eq!(instance_manager.get_pos_table().get_positions().len(), 1);

            gen_draw_calls(&mut app);
            assert!(!app.draw_packet.is_empty());
            let pnu_items: Vec<&DrawItem> = app.draw_packet.get_pnu().values().flatten().collect();
            let pnujw_items: Vec<&DrawItem> =
                app.draw_packet.get_pnujw().values().flatten().collect();
            assert_eq!(pnu_items.len(), 1, "box should produce 1 pnu draw item");
            assert!(
                pnujw_items.is_empty(),
                "box should produce no pnujw draw items"
            );
            assert_eq!(pnu_items[0].get_instances().count(), 1);
        });
    }

    #[test]
    fn render_fox() {
        pollster::block_on(async {
            let mut app = setup_world(TestCases::Fox).await;

            run_frame(
                &mut app,
                &[WorldDeltaKind::AssetDidLoad],
                &[RenderDeltaKind::AssetGPULoaded],
            );
            gen_draw_calls(&mut app);
            assert!(app.draw_packet.is_empty());

            run_frame(
                &mut app,
                &[WorldDeltaKind::EntityDidSpawn],
                &[RenderDeltaKind::EntitySpawn],
            );
            let instance_manager = &app.world.as_ref().unwrap().instance_manager;
            assert_eq!(instance_manager.get_all_instances().len(), 1);
            assert_eq!(instance_manager.get_pos_table().get_positions().len(), 1);
            gen_draw_calls(&mut app);

            assert!(!app.draw_packet.is_empty());
            let pnu_items: Vec<&DrawItem> = app.draw_packet.get_pnu().values().flatten().collect();
            let pnujw_items: Vec<&DrawItem> =
                app.draw_packet.get_pnujw().values().flatten().collect();
            assert!(pnu_items.is_empty(), "fox should produce no pnu draw items");
            assert!(
                !pnujw_items.is_empty(),
                "fox should produce pnujw draw items"
            );
            for item in &pnujw_items {
                assert_eq!(item.get_instances().count(), 1);
            }
        });
    }

    /// The fox is the only spawned instance, so it gets the first slot in the instance arena.
    /// All of its pnujw draw items must have lt_idx == 0.
    #[test]
    fn instance_arena_fox_lt_idx_is_zero() {
        pollster::block_on(async {
            let mut app = setup_world(TestCases::Fox).await;

            run_frame(
                &mut app,
                &[WorldDeltaKind::AssetDidLoad],
                &[RenderDeltaKind::AssetGPULoaded],
            );
            run_frame(
                &mut app,
                &[WorldDeltaKind::EntityDidSpawn],
                &[RenderDeltaKind::EntitySpawn],
            );

            gen_draw_calls(&mut app);

            let pnujw_items: Vec<&DrawItem> =
                app.draw_packet.get_pnujw().values().flatten().collect();
            assert!(!pnujw_items.is_empty(), "fox should have pnujw draw items");
            for item in &pnujw_items {
                assert_eq!(
                    item.get_lt_idx(),
                    0,
                    "first instance allocated in the arena must start at lt_idx 0"
                );
            }
        });
    }

    /// In the fox+box scene the box is spawned first (EntityHandle(0)), so it occupies the
    /// initial local-transform slots in the arena.  The fox (EntityHandle(1)) is allocated
    /// after the box, so its lt_idx must be > 0.
    #[test]
    fn instance_arena_fox_box_fox_lt_idx_nonzero() {
        pollster::block_on(async {
            let mut app = setup_world(TestCases::BoxFox).await;

            run_frame(
                &mut app,
                &[WorldDeltaKind::AssetDidLoad, WorldDeltaKind::AssetDidLoad],
                &[
                    RenderDeltaKind::AssetGPULoaded,
                    RenderDeltaKind::AssetGPULoaded,
                ],
            );
            run_frame(
                &mut app,
                &[
                    WorldDeltaKind::EntityDidSpawn,
                    WorldDeltaKind::EntityDidSpawn,
                ],
                &[RenderDeltaKind::EntitySpawn, RenderDeltaKind::EntitySpawn],
            );

            gen_draw_calls(&mut app);

            let pnujw_items: Vec<&DrawItem> =
                app.draw_packet.get_pnujw().values().flatten().collect();
            assert!(
                !pnujw_items.is_empty(),
                "fox should have pnujw draw items in the fox+box scene"
            );
            for item in &pnujw_items {
                assert!(
                    item.get_lt_idx() > 0,
                    "fox is the second instance allocated; box occupies the start of the arena so fox lt_idx must be > 0"
                );
            }
        });
    }

    #[test]
    fn render_fox_box() {
        pollster::block_on(async {
            let mut app = setup_world(TestCases::BoxFox).await;

            run_frame(
                &mut app,
                &[WorldDeltaKind::AssetDidLoad, WorldDeltaKind::AssetDidLoad],
                &[
                    RenderDeltaKind::AssetGPULoaded,
                    RenderDeltaKind::AssetGPULoaded,
                ],
            );
            gen_draw_calls(&mut app);
            assert!(app.draw_packet.is_empty());

            run_frame(
                &mut app,
                &[
                    WorldDeltaKind::EntityDidSpawn,
                    WorldDeltaKind::EntityDidSpawn,
                ],
                &[RenderDeltaKind::EntitySpawn, RenderDeltaKind::EntitySpawn],
            );
            let instance_manager = &app.world.as_ref().unwrap().instance_manager;
            assert_eq!(instance_manager.get_all_instances().len(), 2);
            assert_eq!(instance_manager.get_pos_table().get_positions().len(), 2);

            gen_draw_calls(&mut app);
            assert!(!app.draw_packet.is_empty());
            let pnu_items: Vec<&DrawItem> = app.draw_packet.get_pnu().values().flatten().collect();
            let pnujw_items: Vec<&DrawItem> =
                app.draw_packet.get_pnujw().values().flatten().collect();
            assert_eq!(pnu_items.len(), 1, "box should produce 1 pnu draw item");
            assert!(
                !pnujw_items.is_empty(),
                "fox should produce pnujw draw items"
            );
            assert_eq!(pnu_items[0].get_instances().count(), 1);
            for item in &pnujw_items {
                assert_eq!(item.get_instances().count(), 1);
            }
        });
    }

    #[test]
    fn render_box_box() {
        pollster::block_on(async {
            let mut app = setup_world(TestCases::Box).await;
            run_frame_unchecked(&mut app);
            run_frame_unchecked(&mut app);

            let instance_manager = &app.world.as_ref().unwrap().instance_manager;

            assert_eq!(instance_manager.get_all_instances().len(), 1);
            assert_eq!(instance_manager.get_pos_table().get_positions().len(), 1);

            let groups = app.world.as_ref().unwrap().instance_manager.get_groups();

            assert_eq!(groups.len(), 1);

            assert_eq!(groups[0].views.len(), 1);

            app.world.as_mut().unwrap().scene.spawn(vec![(
                EntityHandle(0),
                Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::from_translation(cgmath::Vector3 {
                        x: 10.0,
                        y: 10.0,
                        z: 0.0,
                    })
                    .into(),
                }),
            )]);

            run_frame(
                &mut app,
                &[WorldDeltaKind::EntityDidSpawn],
                &[RenderDeltaKind::EntitySpawn],
            );

            gen_draw_calls(&mut app);

            let groups = app.world.as_ref().unwrap().instance_manager.get_groups();

            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].instance_handles.len(), 2);
            assert_eq!(groups[0].views.len(), 1);
            assert_eq!(
                groups[0].instance_handles[0],
                InstanceHandle::mock(ArchetypeId::Position, EntityHandle(0), 0, 0)
            );
            assert_eq!(
                groups[0].instance_handles[1],
                InstanceHandle::mock(ArchetypeId::Position, EntityHandle(0), 1, 0)
            );

            let pnu_items: Vec<&DrawItem> = app.draw_packet.get_pnu().values().flatten().collect();
            let pnujw_items: Vec<&DrawItem> =
                app.draw_packet.get_pnujw().values().flatten().collect();
            assert!(pnujw_items.is_empty());

            assert_eq!(pnu_items.len(), 2);
            assert_eq!(pnu_items[0].get_instances(), 0..1);
            assert_eq!(pnu_items[1].get_instances(), 1..2);
            assert_eq!(pnu_items[0].get_lt_idx(), 0);
            assert_eq!(pnu_items[1].get_lt_idx(), 0);
            assert_eq!(pnu_items[0].get_primitives(), pnu_items[1].get_primitives());
            assert_eq!(pnu_items[0].get_indices(), pnu_items[1].get_indices());

            let world = app.world.as_ref().unwrap();
            assert_eq!(world.scene.spawn_count, 2);

            assert_eq!(world.instance_manager.get_all_instances().len(), 2);
            assert_eq!(
                world.instance_manager.get_pos_table().get_positions().len(),
                2
            );
        })
    }
}
