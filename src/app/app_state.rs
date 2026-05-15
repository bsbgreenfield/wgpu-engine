use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode};

#[allow(unused)]
#[derive(Default)]
pub(super) struct InputController {
    pub key_d_down: bool,
    pub key_w_down: bool,
    pub key_a_down: bool,
    pub key_s_down: bool,
    pub key_q_down: bool,
    pub key_e_down: bool,
    pub key_1_down: bool,
    pub key_2_down: bool,
}
pub struct AppState {
    pub(super) input_controller: InputController,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            input_controller: InputController::default(),
        }
    }
    pub(super) fn handle_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        code: KeyCode,
        is_pressed: bool,
    ) {
        match code {
            KeyCode::Escape => {
                if is_pressed {
                    event_loop.exit();
                }
            }
            KeyCode::KeyA => {
                println!("HELLO");
                self.input_controller.key_a_down = is_pressed;
            }
            _ => {}
        }
    }
}
