//! The `pandemonium-server` program.

/// Runs the server with the options on the command line.
fn main() {
    std::process::exit(pm_server::run_with_arguments(
        "pandemonium-server",
        std::env::args().skip(1),
    ));
}
