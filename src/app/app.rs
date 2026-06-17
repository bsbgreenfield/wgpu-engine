use std::{sync::Arc, time::Instant};

use crate::{
    app::{
        FrameError,
        app_config::AppConfig,
        app_state::AppState,
        renderer::{
            DrawPacket, Instruction, RenderCategory, RenderConstant, RenderPacket,
            renderer::Renderer,
        },
    },
    world::{entity_manager::entity_manager::EntityManager, scene::Scene, world::World},
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
    last_frame_time: Instant,
    pub window: Option<Arc<Window>>,
    pub app_config: Option<AppConfig<'a>>,
    pub world: Option<World>,
    pub renderer: Option<Renderer>,
    pub app_state: AppState,
    pub surface_ready: bool,
    pub render_packet: RenderPacket,
    pub app_commands: Vec<AppCommand>,
}

pub enum AppCommand {
    One,
    Two,
    Three,
}

impl App<'_> {
    pub fn new() -> Self {
        Self {
            last_frame_time: Instant::now(),
            window: None,
            app_config: None,
            app_state: AppState::new(),
            surface_ready: false,
            renderer: None,
            world: None,
            render_packet: RenderPacket::new(),
            app_commands: Vec::with_capacity(100),
        }
    }

    pub fn run_frame(&mut self) -> Result<(), FrameError> {
        use std::sync::atomic::AtomicU64;
        use std::sync::atomic::Ordering;

        static FRAME: AtomicU64 = AtomicU64::new(0);
        let frame = FRAME.fetch_add(1, Ordering::Relaxed);
        if frame % 60 == 0 {
            let im = &self.world.as_ref().unwrap().instance_manager;

            //  eprintln!(
            //      "frame={} active_anims={} app_cmds={} pnu_keys={} pnujw_keys={} indirection={} gt={}",
            //      frame,
            //      im.animation_controller.active_animations.len(),
            //      self.app_commands.len(),
            //      self.render_packet.draw_packet.pnu.len(),
            //      self.render_packet.draw_packet.pnujw.len(),
            //      self.render_packet.draw_packet.indirection_list.len(),
            //      self.render_packet.global_transforms.len(),
            //  );
        }
        if self.app_state.input_controller.key_1_down {
            self.app_commands.push(AppCommand::One);
            self.app_state.input_controller.key_1_down = false;
        }
        if self.app_state.input_controller.key_2_down {
            self.app_commands.push(AppCommand::Two);
            self.app_state.input_controller.key_2_down = false;
        }
        if self.app_state.input_controller.key_3_down {
            self.app_commands.push(AppCommand::Three);
            self.app_state.input_controller.key_3_down = false;
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
            &self.app_config.as_ref().unwrap().device,
        )?;
        self.world
            .as_mut()
            .unwrap()
            .post_frame_update(render_deltas);

        self.world
            .as_ref()
            .unwrap()
            .instance_manager
            .gen_draw_calls(
                &mut self.render_packet,
                &self.world.as_ref().unwrap().entity_manager.asset_manager,
            );

        let render_frame = self
            .world
            .as_ref()
            .unwrap()
            .instance_manager
            .prepare_render_frame(&self.render_packet);

        self.renderer
            .as_mut()
            .unwrap()
            .prepare_frame(render_frame, &self.app_config.as_ref().unwrap().queue);

        if !self.render_packet.draw_packet.is_empty() {
            let _ = self
                .renderer
                .as_ref()
                .unwrap()
                .render(
                    self.app_config.as_ref().unwrap(),
                    &self.world.as_ref().unwrap().camera,
                    &self.render_packet.draw_packet,
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
                let scene = Scene::shared_foxes(self.world.as_mut().unwrap()).unwrap();
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

                self.last_frame_time = Instant::now();
                match self.run_frame() {
                    Ok(_) => {
                        let next_frame =
                            self.last_frame_time + self.app_config.as_ref().unwrap().target_fps;
                        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                            next_frame,
                        ));
                    }
                    Err(FrameError::SurfaceError(_)) => {
                        // let size = self.window.as_ref().unwrap().inner_size();
                        // self.app_config
                        //     .as_mut()
                        //     .unwrap()
                        //     .resize(PhysicalSize::new(size.width, size.height));
                    }
                    Err(e) => {
                        panic!("unable to render! {}", e);
                    }
                }
            }
            _ => (),
        }
    }
    fn new_events(
        &mut self,
        _event_loop: &event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        use winit::event::StartCause;
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn user_event(&mut self, _event_loop: &event_loop::ActiveEventLoop, event: AppConfig<'static>) {
        self.app_config = Some(event);
    }
}
