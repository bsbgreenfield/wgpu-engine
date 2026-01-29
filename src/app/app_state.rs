use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode};

use crate::world::world::World;

pub struct AppState;
impl AppState {
    pub(super) fn handle_key(&self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {}
        }
    }
}
