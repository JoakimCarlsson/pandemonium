//! Application entry point: opens the window and runs the event loop.

mod agent;
mod app;
mod config;
mod desktop;
mod editor;
mod field;
#[allow(
    dead_code,
    reason = "a box of text is offered whole — one line or many — before every box in the window has been moved onto it"
)]
mod input;
#[allow(
    dead_code,
    unused_imports,
    reason = "the keymap is a schema in full before the panes that press it exist"
)]
mod keymap;
mod message;
mod onboarding;
mod panes;
mod picker;
mod prompt;
mod review;
mod terminal;
mod tree;
mod workspace;

use winit::event_loop::{ControlFlow, EventLoop};

use app::{App, Wake};

/// Starts the conductor window from the preferences the last launch left.
fn main() {
    let event_loop = EventLoop::<Wake>::with_user_event()
        .build()
        .expect("event loop creation failed");
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    event_loop
        .run_app(&mut App::restored(proxy))
        .expect("event loop failed");
}
