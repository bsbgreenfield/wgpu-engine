#[cfg(test)]
mod integration_tests {

    use crate::{
        app::{
            app::App,
            app_config::AppConfig,
            app_state::AppState,
            renderer::{Instruction, Operations, RenderUpdateDelta, VMValue, renderer::Renderer},
        },
        asset_manager::AssetHandle,
        world::world::{World, WorldUpdateDelta},
    };

    #[test]
    fn main_test() {
        pollster::block_on(async {
            let mut app = App::new();

            let config = AppConfig::new_headless().await;

            let world = World::new(1.0, &config.device).unwrap();

            let renderer = Renderer::new(&config);

            app.world = Some(world);
            app.app_config = Some(config);
            app.renderer = Some(renderer);
            app.app_state = AppState {};
            app.surface_ready = true;

            let deltas = app.update_world().unwrap();

            assert_eq!(deltas.len(), 1);
            assert!(matches!(deltas[0], WorldUpdateDelta::AssetDidLoad(_)));

            let mut constants = Vec::<VMValue>::new();
            let mut instructions = Vec::<Instruction>::new();
            let asset_handle: Option<AssetHandle>;
            match deltas[0] {
                WorldUpdateDelta::AssetDidLoad(handle) => {
                    asset_handle = Some(handle);
                    let la = app
                        .world
                        .as_ref()
                        .expect("should exist in the asset manager")
                        .get_loaded_asset_of(&handle)
                        .expect("loaded asset should be exactly CPU resident!");
                    // generate bytecode for renderer VM to load an asset
                    constants.push(VMValue::LoadedAsset(la));
                    instructions.push(Instruction::Op(Operations::AddAsset));
                    instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
                }
                _ => panic!("wrong delta"),
            }

            let render_deltas = app
                .renderer
                .as_mut()
                .unwrap()
                .update(
                    constants,
                    instructions,
                    &app.app_config.as_ref().unwrap().queue,
                )
                .unwrap();

            assert!(matches!(
                render_deltas[0],
                RenderUpdateDelta::AssetGPULoaded(_)
            ));

            let draw_packet = app.renderer.as_ref().unwrap().gen_draw_calls_new(
                &app.world.as_ref().unwrap().instance_manager,
                &app.app_config.as_ref().unwrap().queue,
            );
            assert!(draw_packet.is_none());

            //let _ = app
            //    .renderer
            //    .as_ref()
            //    .unwrap()
            //    .render_blank(&app.app_config.as_ref().unwrap());

            app.world
                .as_mut()
                .unwrap()
                .post_frame_update(&render_deltas);

            app.world
                .as_ref()
                .unwrap()
                .asset_manager
                .get_alloc_handle_of(&asset_handle.unwrap());

            // ****************** SECOND FRAME ***********************************
            // *******************************************************************

            let world_deltas = app.update_world().unwrap();

            // assert that an instance spawned
            let instance_manager = &app.world.as_ref().unwrap().instance_manager;
            let instances = instance_manager.get_all_instances();

            assert!(instances.len() == 1);

            let trasnforms = instance_manager.get_pos_table().get_positions();
            assert!(trasnforms.len() == 1);

            let mut constants = Vec::<VMValue>::new();
            let mut instructions = Vec::<Instruction>::new();

            match &world_deltas[0] {
                WorldUpdateDelta::EntityDidSpawn(instance_handle) => {
                    let world = app.world.as_ref().unwrap();
                    let entity_handle = instance_handle.entity_handle.clone();
                    let renderables = world
                        .entity_manager
                        .get_renderables(&entity_handle, &world.asset_manager);

                    instructions.push(Instruction::Op(Operations::SpawnEntityInstance));
                    let assets = App::get_ordered_assets(&renderables);
                    constants.push(VMValue::InstanceHandle(instance_handle.clone()));
                    instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
                    constants.push(VMValue::Renderables(renderables));
                    instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
                    for asset_handle in assets {
                        constants.push(VMValue::LoadedAsset(
                            world
                                .get_loaded_asset_of(&asset_handle)
                                .expect("should be a registered asset"),
                        ));
                        instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
                    }
                }
                _ => panic!("should be an entity did spawn delta"),
            }

            assert!(matches!(constants[0], VMValue::InstanceHandle(_)));
            assert!(matches!(constants[1], VMValue::Renderables(_)));
            assert!(matches!(constants[2], VMValue::LoadedAsset(_)));
            let render_deltas = app
                .renderer
                .as_mut()
                .unwrap()
                .update(
                    constants,
                    instructions,
                    &app.app_config.as_ref().unwrap().queue,
                )
                .unwrap();

            let draw_packet = app
                .renderer
                .as_ref()
                .unwrap()
                .gen_draw_calls_new(&instance_manager, &app.app_config.as_ref().unwrap().queue)
                .unwrap();

            let pnu = draw_packet.get_pnu();
            assert!(pnu.len() == 1);
            for entry in pnu.iter() {
                for item in entry.1 {
                    assert!(item.get_lt_idx() == 0);
                    assert!(item.get_instances().start == 0 && item.get_instances().end == 1);
                    println!("{:?}", item.get_primitives());
                    // TODO: assert prims for box
                }
            }

            app.world
                .as_mut()
                .unwrap()
                .post_frame_update(&render_deltas);
        });
    }
}
