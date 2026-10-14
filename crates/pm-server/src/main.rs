//! The `pandemonium-server` program.

use pm_server::{Settings, run};

/// What the program prints when it is not understood.
const USAGE: &str = "usage: pandemonium-server [--listen ADDRESS] [--token TOKEN] | --version";

/// Reads the options and runs the server.
fn main() {
    let mut settings = Settings {
        listen: format!("127.0.0.1:{}", pm_protocol::DEFAULT_PORT),
        token: std::env::var("PANDEMONIUM_TOKEN").ok(),
    };
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--version" => {
                println!("pandemonium-server {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--listen" => match arguments.next() {
                Some(address) => settings.listen = address,
                None => return fail(),
            },
            "--token" => match arguments.next() {
                Some(token) => settings.token = Some(token),
                None => return fail(),
            },
            _ => return fail(),
        }
    }
    if let Err(error) = run(settings) {
        eprintln!("pandemonium-server: {error}");
        std::process::exit(1);
    }
}

/// Says how the program is used and ends it.
fn fail() {
    eprintln!("{USAGE}");
    std::process::exit(2);
}
