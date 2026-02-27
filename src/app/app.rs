use std::{rc::Rc, sync::Arc};

use crate::{
    app::{
        app_config::AppConfig,
        app_state::AppState,
        render::{
            VMValue,
            renderer::{RenderUpdateDelta, Renderer},
        },
    },
    asset_manager::asset_manager::LoadedAsset,
    world::world::{World, WorldUpdateDelta, WorldUpdateError},
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

#[derive(Debug)]
enum FrameError {
    UpdateError(WorldUpdateError),
    SurfaceError(wgpu::SurfaceError),
    RenderError,
}

impl From<WorldUpdateError> for FrameError {
    fn from(value: WorldUpdateError) -> Self {
        FrameError::UpdateError(value)
    }
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
        for delta in deltas.iter() {
            match delta {
                WorldUpdateDelta::EntityDidLoad(entity_handle) => {
                    // las is actually borrowed for less than the duration of this function call,
                    // as it is dropped before world.post_frame_update()
                    let las: Vec<&LoadedAsset> = self
                        .world
                        .as_ref()
                        .unwrap()
                        .get_loaded_assets_for(*entity_handle);

                    for la in las {
                        constants.push(VMValue::LoadedAsset(la));
                    }
                }
            }
        }
        let render_deltas = self.renderer.as_mut().unwrap().update(
            constants,
            vec![],
            &self.app_config.as_ref().unwrap().queue,
        );

        self.world
            .as_mut()
            .unwrap()
            .post_frame_update(&render_deltas);

        Ok(())
    }

    fn update_world(&mut self) -> Result<Vec<WorldUpdateDelta>, WorldUpdateError> {
        unsafe { self.world.as_mut().unwrap_unchecked().update() }
    }

    fn render(&mut self, constants: Vec<VMValue>) {
        unsafe {
            let renderer = self.renderer.as_mut().unwrap_unchecked();
            todo!()
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
            self.renderer = Some(Renderer::new(self.app_config.as_ref().unwrap()))
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
