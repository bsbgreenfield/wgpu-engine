use std::sync::Arc;

use crate::{
    app::{app_config::AppConfig, app_state::AppState, renderer::Renderer},
    world::world::{World, WorldUpdateError},
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
    pub renderer: Option<Renderer<'a>>,
    pub app_state: AppState,
    surface_ready: bool,
}

impl<'a> App<'a> {
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

    fn run_frame(&mut self) {
        unsafe {
            self.update_world();
            self.render();
        }
    }

    fn update_world(&mut self) -> Result<(), WorldUpdateError> {
        unsafe { self.world.as_mut().unwrap_unchecked().update() }
    }

    fn render(&mut self) {
        unsafe {
            let _ = self.renderer.as_mut().unwrap_unchecked().render(
                &self.app_config.as_ref().unwrap_unchecked().device,
                &self.app_config.as_ref().unwrap_unchecked().surface,
            );
        }
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
            self.renderer = Some(Renderer::new(&self.app_config.as_ref().unwrap().device))
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
                let config = self.app_config.as_ref().unwrap();

                match self
                    .renderer
                    .as_ref()
                    .unwrap()
                    .render(&config.device, &config.surface)
                {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let size = self.window.as_ref().unwrap().inner_size();
                        self.app_config
                            .as_mut()
                            .unwrap()
                            .resize(PhysicalSize::new(size.width, size.height));
                    }
                    Err(e) => {
                        panic!("unable to render! {}", e);
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
