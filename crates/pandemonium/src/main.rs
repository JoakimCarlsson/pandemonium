//! Application entry point: opens the window and runs the event loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use winit::event_loop::{ControlFlow, EventLoop};

use app::App;

/// Opens the window and runs the event loop until it is closed.
fn main() {
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("pandemonium: could not start the event loop: {error}");
            std::process::exit(1);
        }
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    if let Err(error) = event_loop.run_app(&mut App::default()) {
        eprintln!("pandemonium: the event loop failed: {error}");
        std::process::exit(1);
    }
}
