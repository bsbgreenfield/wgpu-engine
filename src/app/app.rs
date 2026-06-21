use std::{sync::Arc, time::Instant};

use crate::{
    app::{
        FrameError,
        app_config::AppConfig,
        app_state::AppState,
        renderer::{Instruction, RenderCategory, RenderConstant, RenderPacket, renderer::Renderer},
    },
    world::{scene::Scene, world::World},
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
    pub world: World,
    pub renderer: Renderer,
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
            renderer: Renderer::new(),
            world: World::new(),
            render_packet: RenderPacket::new(),
            app_commands: Vec::with_capacity(100),
        }
    }

    pub fn run_frame(&mut self) -> Result<(), FrameError> {
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
        let deltas = self.world.update(&mut self.app_commands)?;

        let mut constants = Vec::<RenderConstant>::new();
        let mut instructions = Vec::<Instruction>::new();

        World::gen_bytecode(deltas, &mut instructions, &mut constants);

        let render_deltas = self.renderer.update(
            constants,
            instructions,
            &self.app_config.as_ref().unwrap().queue,
            &self.app_config.as_ref().unwrap().device,
        )?;
        self.world.post_frame_update(render_deltas);

        self.world
            .instance_manager
            .gen_draw_calls(&mut self.render_packet);

        let render_frame = self
            .world
            .instance_manager
            .prepare_render_frame(&self.render_packet);

        self.renderer
            .prepare_frame(render_frame, &self.app_config.as_ref().unwrap().queue);

        if !self.render_packet.draw_packet.is_empty() {
            let _ = self
                .renderer
                .render(
                    self.app_config.as_ref().unwrap(),
                    &self.world.camera,
                    &self.render_packet.draw_packet,
                )
                .map_err(|e| FrameError::RenderError(e));
        } else {
            let _ = self
                .renderer
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

            if !self.world.is_initialized() {
                self.world
                    .init(aspect_ratio, &self.app_config.as_ref().unwrap().device);
                let scene = Scene::multi_box_scene(&mut self.world).unwrap();
                self.world.add_scene(scene);
                self.renderer.init(self.app_config.as_ref().unwrap());
                self.renderer.add_pass(
                    "Opaque Pass".to_string(),
                    vec![RenderCategory::OpaqueStatic, RenderCategory::OpaqueSkinned],
                );
            }
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
