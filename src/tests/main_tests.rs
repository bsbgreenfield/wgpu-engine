#[cfg(test)]
mod integration_tests {

    use crate::{
        animation::animation::AnimationTransformType,
        app::{
            app::App,
            app_config::AppConfig,
            app_state::AppState,
            renderer::{
                DrawItem, Instruction, RenderConstant, RenderUpdateDelta, renderer::Renderer,
            },
        },
        world::{
            entity_manager::{EntityHandle, EntityManager},
            instance_manager::{APosition, ArchetypeId, InstanceHandle},
            scene::Scene,
            world::{World, WorldUpdateDelta},
        },
    };

    enum TestCases {
        Box,
        Fox,
        BoxFox,
        BoxAnimated,
    }

    /// Variant-only mirrors of WorldUpdateDelta — use these to declare what a frame should produce.
    #[derive(Debug)]
    enum WorldDeltaKind {
        AssetDidLoad,
        EntityDidSpawn,
    }

    /// Variant-only mirrors of RenderUpdateDelta — use these to declare what the renderer should emit.
    #[derive(Debug)]
    enum RenderDeltaKind {
        AssetGPULoaded,
        EntitySpawn,
    }

    fn get_bytecode<'a>(
        deltas: Vec<WorldUpdateDelta<'a>>,
    ) -> (Vec<RenderConstant<'a>>, Vec<Instruction>) {
        let mut constants = Vec::<RenderConstant<'a>>::new();
        let mut instructions = Vec::<Instruction>::new();

        World::gen_bytecode(deltas, &mut instructions, &mut constants);

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
            TestCases::BoxAnimated => Scene::box_animated(&mut world).expect("box animated init"),
        };
        world.add_scene(scene);
        app.world = Some(world);
        app.app_config = Some(config);
        app.renderer = Some(renderer);
        app.app_state = AppState::new();
        app.surface_ready = true;

        app
    }

    fn run_frame(
        app: &mut App<'_>,
        expected_world_deltas: &[WorldDeltaKind],
        expected_render_deltas: &[RenderDeltaKind],
    ) {
        let deltas = app
            .world
            .as_mut()
            .unwrap()
            .update(&mut app.app_commands)
            .unwrap_or_else(|e| panic!("{}", e));
        assert_world_deltas(&deltas, expected_world_deltas);

        let (constants, instructions) = get_bytecode(deltas);

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
        let deltas = app
            .world
            .as_mut()
            .unwrap()
            .update(&mut app.app_commands)
            .unwrap_or_else(|e| panic!("{}", e));
        let (constants, instructions) = get_bytecode(deltas);

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

    /// Test that instances that spawned with a global transform have that global transform properly
    /// stored in the archetype table, which is accessible with its instance handle
    #[test]
    fn pos_table_stores_scene_spawn_transforms() {
        pollster::block_on(async {
            use cgmath::SquareMatrix;

            let mut app = setup_world(TestCases::Box).await;
            run_frame_unchecked(&mut app); // asset load
            run_frame_unchecked(&mut app); // entity spawn at identity

            let im = &app.world.as_ref().unwrap().instance_manager;
            let positions = im.get_pos_table().get_positions();
            assert_eq!(positions.len(), 1);

            // box_scene spawns at the identity matrix
            let identity: crate::util::types::GlobalTransform =
                cgmath::Matrix4::<f32>::identity().into();
            let mock_instance_handle_1 =
                InstanceHandle::mock(ArchetypeId::Position, EntityHandle(0), 0, 0);

            let pos_table_ix = im.resolve_idx(&mock_instance_handle_1).unwrap_or_else(|| {
                panic!("instance handle not found in the archetype table. ",);
            });

            assert_eq!(
                positions[pos_table_ix].transform, identity.transform,
                "archetype table must store the identity transform given at spawn"
            );

            // spawn a second instance at a distinct, known translation
            let translation_mat =
                cgmath::Matrix4::<f32>::from_translation(cgmath::Vector3::new(3.0, 5.0, 7.0));
            let translation_gt: crate::util::types::GlobalTransform = translation_mat.into();

            app.world.as_mut().unwrap().scene.spawn(vec![(
                EntityHandle(0),
                Box::new(APosition {
                    position: translation_mat.into(),
                }),
            )]);

            run_frame_unchecked(&mut app);

            let positions = app
                .world
                .as_ref()
                .unwrap()
                .instance_manager
                .get_pos_table()
                .get_positions();
            assert_eq!(positions.len(), 2);
            let mock_instance_handle_2 =
                InstanceHandle::mock(ArchetypeId::Position, EntityHandle(0), 1, 0);

            let pos_table_idx_1 = app
                .world
                .as_ref()
                .unwrap()
                .instance_manager
                .resolve_idx(&mock_instance_handle_1)
                .unwrap_or_else(|| {
                    panic!("instance handle not found in the archetype table. ",);
                });
            let pos_table_idx_2 = app
                .world
                .as_ref()
                .unwrap()
                .instance_manager
                .resolve_idx(&mock_instance_handle_2)
                .unwrap_or_else(|| {
                    panic!("instance handle not found in the archetype table. ",);
                });
            assert_eq!(
                positions[pos_table_idx_1].transform, identity.transform,
                "slot 0 must retain the identity transform from the first spawn"
            );
            assert_eq!(
                positions[pos_table_idx_2].transform, translation_gt.transform,
                "slot 1 must store the translation transform from the second spawn"
            );
        });
    }

    /// The instance range inside each DrawItem is an index into the global-transform buffer that
    /// `prepare_render_frame` produces.  This test verifies that the buffer correctly copies the
    /// instance manager's archetype tables, and  that every DrawItem's range start points at the transform that was
    /// originally given to scene.spawn.
    #[test]
    fn draw_item_instance_range_indexes_render_frame_transform() {
        pollster::block_on(async {
            use cgmath::SquareMatrix;

            let mut app = setup_world(TestCases::Box).await;
            run_frame_unchecked(&mut app); // asset load
            run_frame_unchecked(&mut app); // first entity spawn at identity

            let translation_mat =
                cgmath::Matrix4::<f32>::from_translation(cgmath::Vector3::new(10.0, 20.0, 0.0));

            app.world.as_mut().unwrap().scene.spawn(vec![(
                EntityHandle(0),
                Box::new(APosition {
                    position: translation_mat.into(),
                }),
            )]);

            run_frame_unchecked(&mut app);
            gen_draw_calls(&mut app);

            let (frame_transforms, positions): (
                Vec<crate::util::types::GlobalTransform>,
                Vec<crate::util::types::GlobalTransform>,
            ) = {
                let im = &app.world.as_ref().unwrap().instance_manager;
                let render_frame = im.prepare_render_frame();
                assert_eq!(
                    render_frame.global_transforms.len(),
                    1,
                    "one byte-slice expected: one archetype table"
                );
                // THE GLOBAL TRANSFORMS IN THE RENDER FRAME
                let render_bytes_as_global_transforms: Vec<crate::util::types::GlobalTransform> =
                    bytemuck::cast_slice(render_frame.global_transforms[0]).to_vec();
                // THE GLOBAL TRANSFORMS IN THE ARCH TABLE
                let positions = im.get_pos_table().get_positions();
                (render_bytes_as_global_transforms, positions)
            };

            // should be the same
            assert_eq!(frame_transforms.len(), 2);
            assert_eq!(positions.len(), 2);

            for i in 0..2 {
                assert_eq!(
                    frame_transforms[i].transform, positions[i].transform,
                    "render frame slot {i} must equal pos table slot {i}"
                );
            }

            // Every draw items instance range must point at a transform we actually spawned.
            let identity_gt: crate::util::types::GlobalTransform =
                cgmath::Matrix4::<f32>::identity().into();
            let translation_gt: crate::util::types::GlobalTransform = translation_mat.into();

            let pnu_items: Vec<&DrawItem> = app.draw_packet.get_pnu().values().flatten().collect();
            assert_eq!(
                pnu_items.len(),
                2,
                "expected two pnu draw items for two instances"
            );

            let mut seen_identity = false;
            let mut seen_translation = false;
            for item in pnu_items.iter() {
                let idx = item.get_instances().start as usize;
                if idx == 0 {
                    seen_identity = true;
                    assert_eq!(frame_transforms[idx].transform, identity_gt.transform);
                } else if idx == 1 {
                    seen_translation = true;
                    assert_eq!(frame_transforms[idx].transform, translation_gt.transform);
                }
            }
            assert!(
                seen_identity,
                "no draw item mapped to the identity transform"
            );
            assert!(
                seen_translation,
                "no draw item mapped to the translation transform"
            );
        });
    }

    #[test]
    fn render_two_boxes() {
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

    #[test]
    fn box_animated() {
        pollster::block_on(async {
            let mut app = setup_world(TestCases::BoxAnimated).await;

            // Frame 1: asset loads to GPU
            run_frame(
                &mut app,
                &[WorldDeltaKind::AssetDidLoad],
                &[RenderDeltaKind::AssetGPULoaded],
            );

            // Frame 2: entity spawns
            run_frame(
                &mut app,
                &[WorldDeltaKind::EntityDidSpawn],
                &[RenderDeltaKind::EntitySpawn],
            );

            app.world
                .as_ref()
                .unwrap()
                .instance_manager
                .assert_local_transforms_exist(&InstanceHandle::mock(
                    ArchetypeId::Position,
                    EntityHandle(0),
                    0,
                    0,
                ));

            gen_draw_calls(&mut app);

            let pnu_items: Vec<&DrawItem> = app.draw_packet.get_pnu().values().flatten().collect();
            assert_eq!(pnu_items.len(), 2);
            println!("{:?}", pnu_items[0]);
            println!("{:?}", pnu_items[1]);
            assert_eq!(pnu_items[0].get_lt_idx(), 0);
            assert_eq!(pnu_items[1].get_lt_idx(), 1);

            let instance_manager = &app.world.as_ref().unwrap().instance_manager;
            assert_eq!(instance_manager.get_all_instances().len(), 1);

            let instance_handle =
                InstanceHandle::mock(ArchetypeId::Position, EntityHandle(0), 0, 0);

            // Activate animation 0; this registers an AnimationInstance for the entity
            app.world
                .as_mut()
                .unwrap()
                .instance_manager
                .activate_animation(&instance_handle, 0, None);

            // Verify one animation is now active
            let anim_instances = app
                .world
                .as_ref()
                .unwrap()
                .instance_manager
                .get_active_animations(); // ERROR: add this getter to InstanceManager
            assert_eq!(
                anim_instances.len(),
                1,
                "one animation should be active after activate_animation"
            );
            assert_eq!(anim_instances[0].samples.len(), 2);
            assert_eq!(anim_instances[0].mesh_buffer.len(), 2);

            let anim = app
                .world
                .as_ref()
                .unwrap()
                .instance_manager
                .get_animation_ref(&instance_handle.entity_handle, 0);

            let (channels, samplers) = anim.get_channels_and_samplers();
            assert_eq!(channels.len(), 2);
            assert_eq!(samplers.len(), 2);
            assert!(
                matches!(
                    channels.get(&0).unwrap()[0].1,
                    AnimationTransformType::Translation
                ),
                "{:?} --- {:?}",
                channels.get(&0).unwrap()[0],
                channels.get(&2).unwrap()[0]
            );
            assert!(matches!(
                channels.get(&2).unwrap()[0].1,
                AnimationTransformType::Rotation
            ),);

            assert_eq!(samplers[0].times.len(), 2);
            assert_eq!(samplers[0].transforms.0.len(), 8);
            assert_eq!(samplers[1].times.len(), 4);
            assert_eq!(samplers[1].transforms.0.len(), 12);

            // Frame 3: world.update() calls instance_manager.update(), which drives
            // get_animation_frame and populates the per-instance local-transform buffer
            run_frame_unchecked(&mut app);

            let im = &app.world.as_ref().unwrap().instance_manager;
            let render_frame = im.prepare_render_frame();

            assert_eq!(
                render_frame.rigid_animation_data.len(),
                1,
                "one active animation should produce one AnimationUpdate in the render frame"
            );

            assert_eq!(render_frame.rigid_animation_data[0].transforms.len(), 128);

            let anim_update = &render_frame.rigid_animation_data[0];
            assert!(
                !anim_update.transforms.is_empty(),
                "animation transforms buffer must be non-empty after the first update tick"
            );

            // The transforms slice is tightly packed Mat4<f32> values (64 bytes each).
            // Verify the byte count is a valid multiple of the matrix size so the GPU
            // upload won't produce partial matrices.
            assert_eq!(
                anim_update.transforms.len() % std::mem::size_of::<crate::util::types::Mat4F32>(),
                0,
                "transforms byte length must be a multiple of Mat4F32 size"
            );

            // test the animation at various times
            app.world
                .as_mut()
                .unwrap()
                .instance_manager
                .run_animations(2.49);
            assert!(
                app.world
                    .as_ref()
                    .unwrap()
                    .instance_manager
                    .get_active_animations()
                    .len()
                    > 0
            );
            let bsm = app
                .world
                .as_ref()
                .unwrap()
                .instance_manager
                .get_buffer_slot_map(0);
            let mesh1 = &app
                .world
                .as_ref()
                .unwrap()
                .instance_manager
                .get_active_animations()[0]
                .mesh_buffer[bsm[0]];
            //    .buffer[0][3][1];
            assert!(
                (mesh1[3][1] - 2.52).abs() < 0.1,
                "mesh 0 should be near peak (y ≈ 2.52) at t=2.6s, got {mesh1:?}"
            );
            app.world
                .as_mut()
                .unwrap()
                .instance_manager
                .run_animations(3.5);

            let mesh0_y_descending = app
                .world
                .as_ref()
                .unwrap()
                .instance_manager
                .get_active_animations()[0]
                .mesh_buffer[0][3][1];
            assert!(
                mesh0_y_descending < 2.0,
                "mesh 0 should be descending (y ≈ 0.43) at t=3.5s, got {mesh0_y_descending}"
            );
        })
    }
}
