//! Reading the command line shared by every program that can run the server.

use crate::{Settings, run};

/// What a program prints when its options are not understood.
const USAGE: &str = "[--listen ADDRESS] [--token TOKEN] | --version | --index-languages FOLDER";

/// Runs the server as the program `program` was asked to, from its command-line `arguments` without the program name, and returns the process exit code.
///
/// Every panic of the process is recorded in the app's crash log, and while the server runs its traces and metrics are sent when the `OTEL_*` variables ask for it and the build can.
pub fn run_with_arguments(program: &str, mut arguments: impl Iterator<Item = String>) -> i32 {
    crate::diagnostics::record_crashes(crate::diagnostics::SERVER);
    let mut settings = Settings {
        listen: format!("127.0.0.1:{}", pm_protocol::DEFAULT_PORT),
        token: std::env::var("PANDEMONIUM_TOKEN").ok(),
    };
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--version" => {
                println!("{program} {}", env!("CARGO_PKG_VERSION"));
                return 0;
            }
            "--index-languages" => {
                let Some(folder) = arguments.next() else {
                    return misuse(program);
                };
                return match pm_lsp::build_index(std::path::Path::new(&folder)) {
                    Ok(count) => {
                        println!("{program}: listed {count} languages in {folder}/languages.json");
                        0
                    }
                    Err(reason) => {
                        eprintln!("{program}: {reason}");
                        1
                    }
                };
            }
            "--listen" => match arguments.next() {
                Some(address) => settings.listen = address,
                None => return misuse(program),
            },
            "--token" => match arguments.next() {
                Some(token) => settings.token = Some(token),
                None => return misuse(program),
            },
            _ => return misuse(program),
        }
    }
    let telemetry = crate::diagnostics::start_telemetry(crate::diagnostics::SERVER);
    let code = match run(settings) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{program}: {error}");
            1
        }
    };
    if let Some(telemetry) = telemetry {
        telemetry.shutdown();
    }
    code
}

/// Says how `program` is used and returns the exit code for a misuse.
fn misuse(program: &str) -> i32 {
    eprintln!("usage: {program} {USAGE}");
    2
}
