//! Application entry point: opens the window and runs the event loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod keys;
mod scene;
mod snapshot;
mod view;
mod work;

use std::path::Path;

use winit::event_loop::{ControlFlow, EventLoop};

use app::App;
use snapshot::Shot;

/// Opens the window and runs the event loop until it is closed, or, given `--server`, runs the server
/// on this machine with the options that follow, or, given
/// `--snapshot <path> [elapsed_ms [x y [focus [light [screen]]]]]`, writes one frame to a PNG and leaves.
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("--server") {
        std::process::exit(pm_server::run_with_arguments(
            "pandemonium --server",
            arguments.into_iter().skip(1),
        ));
    }
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
        let appearance = match arguments.get(6).map(String::as_str) {
            Some("light") => pm_theme::Appearance::Light,
            _ => pm_theme::Appearance::Dark,
        };
        let shot = match arguments.get(7).map(String::as_str) {
            Some("project") => Shot::Onboarding(pm_pages::Step::Project),
            Some("agents") => Shot::Onboarding(pm_pages::Step::Agents),
            Some("workspace") => Shot::Workspace,
            Some("diff") => Shot::Diff,
            Some("terminal") => Shot::Terminal,
            Some("new") => Shot::New,
            Some("empty") => Shot::Empty,
            Some("history") => Shot::History,
            Some("folded") => Shot::Folded,
            Some("projectmenu") => Shot::ProjectMenu,
            Some("dragging") => Shot::Dragging,
            Some("welcome") => Shot::Welcome,
            Some("wrapped") => Shot::Wrapped,
            Some("slashed") => Shot::Slashed,
            Some("dollar") => Shot::Dollar,
            Some("scrolling") => Shot::Scrolling,
            Some("placing") => Shot::Placing,
            Some("live") => Shot::Live,
            Some("asking") => Shot::Asking,
            Some("queued") => Shot::Queued,
            Some("picking") => Shot::Picking,
            Some("expanded") => Shot::Expanded,
            Some("slashing") => Shot::Slashing,
            Some("missing") => Shot::Missing,
            Some("attached") => Shot::Attached,
            Some("unfolded") => Shot::Unfolded,
            _ => Shot::Onboarding(pm_pages::Step::Welcome),
        };
        if let Err(error) = snapshot::write(
            Path::new(path),
            elapsed_ms,
            pointer,
            focused,
            appearance,
            shot,
        ) {
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
