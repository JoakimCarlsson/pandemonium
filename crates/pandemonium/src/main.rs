//! Application entry point: opens the window and runs the event loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod app;
mod arrival;
mod build_info;
mod config;
mod debug;
mod desktop;
mod editor;
mod emblem;
mod excerpts;
mod field;
mod health;
mod image;
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
mod markdown;
mod message;
mod notice;
mod notification;
mod onboarding;
mod orchestration;
mod outline;
mod panel;
mod panes;
mod picker;
mod project_groups;
mod prompt;
mod release;
mod review;
mod settings;
mod shell_path;
mod tasks;
mod terminal;
mod testing;
mod theme;
mod tree;
mod workspace;

use winit::event_loop::{ControlFlow, EventLoop};

use app::{App, Wake};

/// Starts the conductor window from the preferences the last launch left,
/// or prints the version and leaves when asked for it with `--version`.
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("{}", build_info::description());
        return;
    }
    shell_path::adopt();
    let event_loop = EventLoop::<Wake>::with_user_event()
        .build()
        .expect("event loop creation failed");
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    event_loop
        .run_app(&mut App::restored(proxy))
        .expect("event loop failed");
}
