//! Application entry point: opens the window and runs the event loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod keys;
mod scene;
mod snapshot;

use std::path::Path;

use winit::event_loop::{ControlFlow, EventLoop};

use app::App;

/// Opens the window and runs the event loop until it is closed, or, given
/// `--snapshot <path> [elapsed_ms [x y [focus]]]`, writes one frame to a PNG and leaves.
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("--snapshot") {
        let Some(path) = arguments.get(1) else {
            eprintln!("pandemonium: --snapshot needs a path");
            std::process::exit(2);
        };
        let elapsed_ms = arguments
            .get(2)
            .and_then(|value| value.parse().ok())
            .unwrap_or(700.0);
        let pointer = arguments
            .get(3)
            .zip(arguments.get(4))
            .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)));
        let focused = arguments
            .get(5)
            .and_then(|value| value.parse().ok())
            .map(pm_input::FocusId);
        if let Err(error) = snapshot::write(Path::new(path), elapsed_ms, pointer, focused) {
            eprintln!("pandemonium: {error}");
            std::process::exit(1);
        }
        return;
    }

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
