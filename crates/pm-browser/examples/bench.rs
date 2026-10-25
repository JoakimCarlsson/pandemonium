//! Loads a URL offscreen for a few seconds and reports how often the page paints.
//!
//! `bench <url> <seconds> <shared_texture:0|1> [switch...]`: the measure of
//! whether the page can paint at display rate.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use pm_browser::{Engine, Options, View, run_helper};

/// Runs the engine for `seconds`, pumping it.
fn run(engine: &Engine, seconds: f64) {
    let until = Instant::now() + Duration::from_secs_f64(seconds);
    while Instant::now() < until {
        engine.pump();
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Runs the engine, lets the page paint, and prints paints per second.
fn main() -> ExitCode {
    if let Some(code) = run_helper() {
        return ExitCode::from(code.max(0) as u8);
    }
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [url, seconds, shared, switches @ ..] = arguments.as_slice() else {
        eprintln!("usage: bench <url> <seconds> <shared_texture:0|1> [switch...]");
        return ExitCode::FAILURE;
    };
    let cache = std::env::current_dir()
        .expect("a working directory")
        .join("build/pm-browser-cache");
    let options = Options {
        switches: switches.to_vec(),
        shared_texture: shared == "1",
        debugging_port: None,
    };
    let engine = match Engine::start(&cache, &options) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let view = match View::open(&engine, url, 1280, 800, 1.0) {
        Ok(view) => view,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    run(&engine, 3.0);
    let before = view.stats();
    let seconds: f64 = seconds.parse().unwrap_or(5.0);
    run(&engine, seconds);
    let after = view.stats();
    println!(
        "software {:.1}/s  shared_texture {:.1}/s  title {:?}",
        (after.software - before.software) as f64 / seconds,
        (after.shared_texture - before.shared_texture) as f64 / seconds,
        view.title(),
    );
    ExitCode::SUCCESS
}
