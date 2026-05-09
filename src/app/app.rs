use std::sync::Arc;

use crate::{
    app::{
        FrameError,
        app_config::AppConfig,
        app_state::AppState,
        renderer::{DrawPacket, Instruction, RenderCategory, RenderConstant, renderer::Renderer},
    },
    world::{entity_manager::EntityManager, scene::Scene, world::World},
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
    pub renderer: Option<Renderer>,
    pub app_state: AppState,
    pub surface_ready: bool,
    pub draw_packet: DrawPacket,
    pub app_commands: Vec<AppCommand>,
}

pub enum AppCommand {
    DoSomething,
}

impl App<'_> {
    pub fn new() -> Self {
        Self {
            window: None,
            app_config: None,
            app_state: AppState::new(),
            surface_ready: false,
            renderer: None,
            world: None,
            draw_packet: DrawPacket::default(),
            app_commands: Vec::with_capacity(100),
        }
    }

    pub fn run_frame(&mut self) -> Result<(), FrameError> {
        if self.app_state.input_controller.key_a_down {
            self.app_commands.push(AppCommand::DoSomething);
            self.app_state.input_controller.key_a_down = false;
        }
        let deltas = self
            .world
            .as_mut()
            .unwrap()
            .update(&mut self.app_commands)?;

        let mut constants = Vec::<RenderConstant>::new();
        let mut instructions = Vec::<Instruction>::new();

        World::gen_bytecode(deltas, &mut instructions, &mut constants);

        let render_deltas = self.renderer.as_mut().unwrap().update(
            constants,
            instructions,
            &self.app_config.as_ref().unwrap().queue,
        )?;
        self.world
            .as_mut()
            .unwrap()
            .post_frame_update(render_deltas);

        self.world
            .as_ref()
            .unwrap()
            .instance_manager
            .gen_draw_calls(&mut self.draw_packet);

        let render_frame = self
            .world
            .as_ref()
            .unwrap()
            .instance_manager
            .prepare_render_frame();

        self.renderer
            .as_mut()
            .unwrap()
            .prepare_frame(render_frame, &self.app_config.as_ref().unwrap().queue);

        if !self.draw_packet.is_empty() {
            let _ = self
                .renderer
                .as_ref()
                .unwrap()
                .render(
                    self.app_config.as_ref().unwrap(),
                    &self.world.as_ref().unwrap().camera,
                    &self.draw_packet,
                )
                .map_err(|e| FrameError::RenderError(e));
        } else {
            let _ = self
                .renderer
                .as_ref()
                .unwrap()
                .render_blank(self.app_config.as_ref().unwrap())
                .map_err(|e| FrameError::RenderError(e));
        }

        Ok(())
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

            if self.world.is_none() {
                let entity_manager = EntityManager::new();

                self.world = Some(
                    World::new(
                        aspect_ratio,
                        entity_manager,
                        &self.app_config.as_ref().unwrap().device,
                    )
                    .unwrap(),
                );
                let scene = Scene::box_animated(self.world.as_mut().unwrap()).unwrap();
                self.world.as_mut().unwrap().add_scene(scene);
                //  #[cfg(test)]
                //  {
                //      let mut scene = Scene::fox_box(&mut self.world.as_mut().unwrap()).unwrap();
                //      scene.set_load_level(crate::world::scene::SceneLoadLevel::GPU);
                //      self.world.as_mut().unwrap().add_scene(scene);
                //  }
            }
            let mut renderer = Renderer::new(&self.app_config.as_ref().unwrap());
            renderer.add_pass(
                "Opaque Pass".to_string(),
                vec![RenderCategory::OpaqueStatic, RenderCategory::OpaqueSkinned],
            );

            self.renderer = Some(renderer)
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
                    Ok(_) => {
                        self.window.as_ref().unwrap().request_redraw();
                    }
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
