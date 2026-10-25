//! `pandemonium-browser`: the program that holds the browser engine for the window.
//!
//! The window starts it with `--control <socket> --surfaces <folder> --profile
//! <folder>`, connects over the socket, and sends it requests, one JSON object
//! to a line; it answers with reports, and paints pages into files in the
//! surfaces folder that the window maps. It ends when the window hangs up.

#[cfg(target_os = "linux")]
mod agent;
#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod bridge;
#[cfg(target_os = "linux")]
mod cdp;
#[cfg(target_os = "linux")]
mod engine;
#[cfg(target_os = "linux")]
mod handler;
#[cfg(target_os = "linux")]
mod pages;
#[cfg(target_os = "linux")]
mod pick;
#[cfg(target_os = "linux")]
mod run;

/// Serves the window until it hangs up.
#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    run::run()
}

/// Says the browser is not built for this system.
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("pandemonium-browser: the embedded browser is not available on this system yet");
    std::process::exit(1);
}
