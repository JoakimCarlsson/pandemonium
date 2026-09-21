//! Application entry point: opens the window and runs the event loop.

mod app;
mod config;
#[allow(
    dead_code,
    unused_imports,
    reason = "the keymap is a schema in full before the panes that press it exist"
)]
mod keymap;
mod onboarding;

use winit::event_loop::{ControlFlow, EventLoop};

use app::App;

/// Starts the conductor window from the preferences the last launch left.
fn main() {
    let event_loop = EventLoop::new().expect("event loop creation failed");
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop
        .run_app(&mut App::restored())
        .expect("event loop failed");
}
