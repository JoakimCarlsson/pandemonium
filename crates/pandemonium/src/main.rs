//! Application entry point: opens the window and runs it, or runs the server,
//! or writes a snapshot.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod file_dialog;
mod notify;
mod performance;
mod snapshot;
mod snapshot_appearance;
mod window;

/// Opens the window and runs it until it is closed; given `--version`, prints
/// the version and leaves; given `--server`, runs the server on this machine
/// with the options that follow; given
/// `--snapshot <path> [WIDTHxHEIGHT] [light|live|settings-<section>[-narrow]|picker] [words]`,
/// writes the window, or a section of the settings window, to a PNG and leaves.
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("--version") => println!("pandemonium {}", env!("CARGO_PKG_VERSION")),
        Some("--server") => std::process::exit(pm_server::run_with_arguments(
            "pandemonium --server",
            arguments.into_iter().skip(1),
        )),
        Some("--snapshot") => std::process::exit(snapshot::run(&arguments[1..])),
        _ => {
            pm_update::forget_previous();
            window::run();
        }
    }
}
