pub mod app;
pub mod asset_manager;
pub mod tests;
pub mod util;
pub mod world;

use anyhow::Result;
use winit::event_loop::{self, EventLoop};
pub fn run() -> Result<()> {
    let event_loop = EventLoop::with_user_event().build()?;
    event_loop.set_control_flow(event_loop::ControlFlow::Poll);

    let mut app = app::app::App::new();
    event_loop.run_app(&mut app).unwrap();
    Ok(())
}
