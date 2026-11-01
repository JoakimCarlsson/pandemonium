//! Optional measurement of where launch time goes, enabled by `PANDEMONIUM_STARTUP_TIMINGS=1`.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// When the first mark was made, which is taken as the moment the process began.
static EPOCH: OnceLock<Instant> = OnceLock::new();

/// Whether `PANDEMONIUM_STARTUP_TIMINGS=1` asked for the marks to be reported.
static ENABLED: OnceLock<bool> = OnceLock::new();

/// Whether the marks are to be reported, read from the environment once.
fn enabled() -> bool {
    *ENABLED.get_or_init(|| {
        std::env::var("PANDEMONIUM_STARTUP_TIMINGS").is_ok_and(|value| value == "1")
    })
}

/// Starts the clock the marks are measured from.
pub fn begin() {
    EPOCH.get_or_init(Instant::now);
}

/// Reports `phase` with the milliseconds since `begin` to the standard error stream when measuring was asked for.
pub fn mark(phase: &str) {
    if !enabled() {
        return;
    }
    let Some(epoch) = EPOCH.get() else {
        return;
    };
    eprintln!(
        "startup: {phase:<24} {:>8.1} ms",
        epoch.elapsed().as_secs_f64() * 1000.0
    );
}

/// Whether the first presented frame has been reported.
static FRAME_REPORTED: AtomicBool = AtomicBool::new(false);

/// Reports the first frame that reached the screen, and ignores every later one.
pub fn frame_presented() {
    if !FRAME_REPORTED.swap(true, Ordering::Relaxed) {
        mark("first-frame-presented");
    }
}

/// Whether the sessions of this machine have been reported as listed.
static SESSIONS_REPORTED: AtomicBool = AtomicBool::new(false);

/// Reports the first listing of the sessions on this machine, which is when the workspace holds what the server keeps, and ignores every later one.
pub fn sessions_listed() {
    if !SESSIONS_REPORTED.swap(true, Ordering::Relaxed) {
        mark("sessions-listed");
    }
}

/// Whether the first scene has been reported.
static SCENE_REPORTED: AtomicBool = AtomicBool::new(false);

/// Reports the first scene built and painted into the draw list, before it is submitted, and ignores every later one.
pub fn scene_painted() {
    if !SCENE_REPORTED.swap(true, Ordering::Relaxed) {
        mark("first-scene-built");
    }
}
