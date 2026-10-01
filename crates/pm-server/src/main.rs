//! The headless machine endpoint, serving only operating system access.

/// Runs the endpoint on its standard pipes or prints its version.
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--version") => println!("pandemonium-server {}", env!("CARGO_PKG_VERSION")),
        Some("--stdio") => {
            if let Err(error) = pm_host::serve(std::io::stdin(), std::io::stdout()) {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("usage: pandemonium-server --stdio | --version");
            std::process::exit(2);
        }
    }
}
