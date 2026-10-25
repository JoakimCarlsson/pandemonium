//! The browser program's life: connecting to the window, starting the engine and serving requests until the window hangs up.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::{TryRecvError, channel};
use std::time::Duration;

use pm_browser::wire::{Event, Request};

use crate::bridge::Bridge;
use crate::engine::{Engine, run_helper};
use crate::handler::Out;
use crate::pages::Pages;

/// The value after `name` in the arguments.
fn option(arguments: &[String], name: &str) -> Option<PathBuf> {
    let at = arguments.iter().position(|argument| argument == name)?;
    arguments.get(at + 1).map(PathBuf::from)
}

/// Runs the engine for the window until it hangs up or asks to shut down.
pub(crate) fn run() -> ExitCode {
    if let Some(code) = run_helper() {
        return ExitCode::from(code.max(0) as u8);
    }
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (Some(control), Some(surfaces), Some(profile)) = (
        option(&arguments, "--control"),
        option(&arguments, "--surfaces"),
        option(&arguments, "--profile"),
    ) else {
        eprintln!("pandemonium-browser: started without --control, --surfaces and --profile");
        return ExitCode::FAILURE;
    };
    let Ok(stream) = UnixStream::connect(&control) else {
        eprintln!("pandemonium-browser: cannot reach the window");
        return ExitCode::FAILURE;
    };
    let Ok(reader) = stream.try_clone() else {
        return ExitCode::FAILURE;
    };
    let out = Out::new(Box::new(stream));
    let _ = std::fs::create_dir_all(&profile);
    let engine = match Engine::start(&profile) {
        Ok(engine) => engine,
        Err(message) => {
            out.send(&Event::Failed { message });
            return ExitCode::FAILURE;
        }
    };
    out.send(&Event::Ready);
    let (sender, receiver) = channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut text = String::new();
        loop {
            text.clear();
            match reader.read_line(&mut text) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if let Ok(request) = serde_json::from_str::<Request>(&text)
                && sender.send(request).is_err()
            {
                break;
            }
        }
    });
    let (wish_sender, wishes) = channel();
    let timers = wish_sender.clone();
    let bridge = match Bridge::start(wish_sender) {
        Ok(bridge) => bridge,
        Err(message) => {
            out.send(&Event::Failed { message });
            return ExitCode::FAILURE;
        }
    };
    let mut pages = Pages::new(out, surfaces, bridge, timers);
    'running: loop {
        engine.pump();
        while let Ok(wish) = wishes.try_recv() {
            pages.wish(wish);
        }
        loop {
            match receiver.try_recv() {
                Ok(request) => {
                    if !pages.handle(request) {
                        break 'running;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'running,
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    engine.settle(Duration::from_millis(300));
    ExitCode::SUCCESS
}
