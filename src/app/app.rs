use std::sync::Arc;

use crate::{
    app::{
        FrameError,
        app_config::AppConfig,
        app_state::AppState,
        renderer_new::{Instruction, Operations, VMValue, renderer_new::RendererNew},
    },
    world::{
        WorldUpdateError,
        world::{World, WorldUpdateDelta},
    },
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop,
    keyboard::PhysicalKey,
    window::Window,
};

pub struct App<'a> {
    pub window: Option<Arc<Window>>,
    pub app_config: Option<AppConfig<'a>>,
    pub world: Option<World>,
    pub renderer: Option<RendererNew>,
    pub app_state: AppState,
    surface_ready: bool,
}

impl App<'_> {
    pub fn new() -> Self {
        Self {
            window: None,
            app_config: None,
            app_state: AppState,
            surface_ready: false,
            renderer: None,
            world: None,
        }
    }

    fn run_frame<'frame>(&'frame mut self) -> Result<(), FrameError> {
        let deltas = self.update_world()?;
        let mut constants = Vec::<VMValue<'frame>>::new();
        let mut instructions = Vec::<Instruction>::new();
        for delta in deltas.iter() {
            match delta {
                WorldUpdateDelta::AssetDidLoad(asset_handle) => {
                    // it makes no sense to emit an "AssetDidLoad" event if either
                    // 1. the asset didn't load to the CPU
                    // 2. the asset is alread GPU resident.
                    // So this is a panic
                    let la = self
                        .world
                        .as_ref()
                        .unwrap()
                        .get_loaded_asset_of(&asset_handle)
                        .expect("loaded asset should be exactly CPU resident!");
                    // generate bytecode for renderer VM to load an asset
                    constants.push(VMValue::LoadedAsset(la));
                    instructions.push(Instruction::Op(Operations::AddAsset));
                    instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
                }

                WorldUpdateDelta::EntityDidSpawn(instance_handle) => {
                    let world = self.world.as_ref().unwrap();
                    let entity_handle = world.instance_manager.entity_of(&instance_handle);
                    let renderables = world.entity_manager.get_renderables(&entity_handle);

                    constants.push(VMValue::InstanceHandle(instance_handle.clone()));
                    constants.push(VMValue::Renderables(renderables));

                    instructions.push(Instruction::Op(Operations::SpawnEntityInstance));
                    instructions.push(Instruction::ConstIdx((constants.len() - 2) as u8));
                    instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
                }
                WorldUpdateDelta::EntityDidLoad(_entity_handle) => {
                    // TODO spawn based on user input or scene state
                }
            }
        }
        let render_deltas = self.renderer.as_mut().unwrap().update(
            constants,
            vec![],
            &self.app_config.as_ref().unwrap().queue,
        )?;

        let draw_packet = self
            .renderer
            .as_ref()
            .unwrap()
            .gen_draw_calls_new(
                &self.world.as_ref().unwrap().instance_manager,
                &self.app_config.as_ref().unwrap().queue,
            )
            .expect("Draw packet should have at least one instance");
        let _ = self
            .renderer
            .as_ref()
            .unwrap()
            .render(self.app_config.as_ref().unwrap(), draw_packet)
            .map_err(|e| FrameError::RenderError(e));

        self.world
            .as_mut()
            .unwrap()
            .post_frame_update(&render_deltas);

        Ok(())
    }

    fn update_world(&mut self) -> Result<Vec<WorldUpdateDelta>, WorldUpdateError> {
        unsafe { self.world.as_mut().unwrap_unchecked().update() }
    }
}

impl ApplicationHandler<AppConfig<'static>> for App<'_> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_none() {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes().with_inner_size(PhysicalSize::new(1500, 1500)),
                    )
                    .unwrap(),
            );
            self.window = Some(window);
            self.app_config = Some(
                pollster::block_on(AppConfig::new(self.window.as_ref().unwrap().clone())).unwrap(),
            );
            let aspect_ratio: f32 = self.app_config.as_ref().unwrap().get_aspect_ratio();
            let world =
                World::new(aspect_ratio, &self.app_config.as_ref().unwrap().device).unwrap();
            self.world = Some(world);
            self.renderer = Some(RendererNew::new(&self.app_config.as_ref().unwrap().device))
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => self
                .app_state
                .handle_key(event_loop, code, key_state.is_pressed()),
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(physical_size) => {
                self.app_config.as_mut().unwrap().resize(physical_size);
                self.surface_ready = true;
            }
            WindowEvent::RedrawRequested => {
                if !self.surface_ready {
                    return;
                }

                match self.run_frame() {
                    Ok(_) => {}
                    Err(FrameError::SurfaceError(_)) => {
                        // let size = self.window.as_ref().unwrap().inner_size();
                        // self.app_config
                        //     .as_mut()
                        //     .unwrap()
                        //     .resize(PhysicalSize::new(size.width, size.height));
                    }
                    Err(e) => {
                        panic!("unable to render! {:?}", e);
                    }
                }
            }
            _ => (),
        }
    }

    fn user_event(&mut self, _event_loop: &event_loop::ActiveEventLoop, event: AppConfig<'static>) {
        self.app_config = Some(event);
    }
}
