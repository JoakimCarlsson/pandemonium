//! Manual CPU benchmarks of interactions on a heavy workspace, replayed
//! through the headless rig the way the event loop feeds the app: dragging
//! each handle between panels, moving the pointer, scrolling, typing and an
//! agent streaming its reply. Each prints, per frame, the time the main
//! window's interface spent composing, laying out and painting, and the
//! whole turn of the loop.
//!
//! ```sh
//! CARGO_TARGET_DIR=build cargo test -p pandemonium benchmark_interactions -- --ignored --nocapture --test-threads=1
//! ```

use pm_components::areas::{ComposerEvent, WindowEvent};
use pm_components::target::Target;
use std::time::{Duration, Instant};

use pm_pages::Screen;
use rustle::PointerButton;

use crate::app::headless::{Rig, SIZE};

/// How many frames each interaction is measured over.
const FRAMES: usize = 100;

/// The rig on the heavy workspace, at the snapshots' size.
fn heavy() -> Rig {
    Rig::new(Screen::Workspace(crate::snapshot::heavy_workspace()), SIZE)
}

/// Per-frame samples of one interaction, in milliseconds.
#[derive(Default)]
struct Samples {
    /// Composing what changed.
    compose: Vec<f64>,
    /// Measuring and painting the tree.
    layout: Vec<f64>,
    /// The whole turn: the app's handling, composing, layout and painting.
    total: Vec<f64>,
}

impl Samples {
    /// Runs `frame` on `rig` and records its times.
    fn measure(&mut self, rig: &mut Rig, frame: impl FnOnce(&mut Rig)) {
        rig.take_spent();
        let started = Instant::now();
        frame(rig);
        let total = started.elapsed();
        let spent = rig.take_spent();
        self.compose.push(milliseconds(spent.compose));
        self.layout.push(milliseconds(spent.layout));
        self.total.push(milliseconds(total));
    }

    /// Prints the medians and the slow tail of every stage, under `name`.
    fn report(mut self, name: &str) {
        eprintln!(
            "{name:<28} compose {}  layout+paint {}  total {}  (ms p50/p95/max over {} frames)",
            summary(&mut self.compose),
            summary(&mut self.layout),
            summary(&mut self.total),
            self.total.len()
        );
    }
}

/// Fractional milliseconds of `duration`.
fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// The median, 95th percentile and largest of `samples`.
fn summary(samples: &mut [f64]) -> String {
    samples.sort_by(f64::total_cmp);
    let at = |percent: usize| samples[(samples.len() * percent).div_ceil(100).max(1) - 1];
    format!(
        "{:6.2}/{:6.2}/{:6.2}",
        at(50),
        at(95),
        samples[samples.len() - 1]
    )
}

/// Drags the handle `id` by one pixel a frame, `FRAMES` frames to and fro
/// along its axis, measuring each move.
fn drag_handle(rig: &mut Rig, id: Target, name: &str) {
    let from = rig.center(id);
    let across = rig.read(|screen| {
        screen
            .sash(id, SIZE)
            .map(|(_, grows)| grows.by((0.0, 0.0), (1.0, 0.0)) != 0.0)
    });
    rig.move_to(from);
    rig.button_in(None, PointerButton::Primary, true, from);
    let mut samples = Samples::default();
    for step in 1..=FRAMES {
        let reach = if step <= FRAMES / 2 {
            step
        } else {
            FRAMES - step
        } as f32;
        let at = if across == Some(true) {
            (from.0 - reach, from.1)
        } else {
            (from.0, from.1 - reach)
        };
        samples.measure(rig, |rig| rig.move_to(at));
    }
    rig.button_in(None, PointerButton::Primary, false, from);
    samples.report(name);
}

/// A handle between two panes of the heavy workspace's split, one not of
/// the panels around them.
fn pane_seam(rig: &Rig) -> Target {
    let fixed = [
        Target::Window(WindowEvent::SidebarSash),
        Target::Window(WindowEvent::ReviewSash),
        Target::Window(WindowEvent::PlanSash),
        Target::Window(WindowEvent::TerminalSash),
    ];
    rig.tagged()
        .iter()
        .map(|node| node.id)
        .find(|id| !fixed.contains(id) && rig.read(|screen| screen.sash(*id, SIZE)).is_some())
        .expect("the panes have a seam")
}

/// Measures dragging the handles of the sidebar, the review panel and a pane seam.
#[test]
#[ignore = "manual CPU benchmark: cargo test -p pandemonium benchmark_interactions -- --ignored --nocapture --test-threads=1"]
fn benchmark_interactions_drag() {
    let mut rig = heavy();
    drag_handle(
        &mut rig,
        Target::Window(WindowEvent::SidebarSash),
        "drag sidebar handle",
    );
    drag_handle(
        &mut rig,
        Target::Window(WindowEvent::ReviewSash),
        "drag review handle",
    );
    let seam = pane_seam(&rig);
    drag_handle(&mut rig, seam, "drag pane seam");
}

/// Measures moving the pointer across the conversation and the panes.
#[test]
#[ignore = "manual CPU benchmark: cargo test -p pandemonium benchmark_interactions -- --ignored --nocapture --test-threads=1"]
fn benchmark_interactions_pointer() {
    let mut rig = heavy();
    let mut hover = Samples::default();
    let (x, top) = rig.center(Target::Window(WindowEvent::SidebarSash));
    for step in 0..FRAMES {
        let at = (
            x + 200.0 + (step % 50) as f32 * 8.0,
            top - 200.0 + step as f32 * 4.0,
        );
        hover.measure(&mut rig, |rig| rig.move_to(at));
    }
    hover.report("move pointer");
}

/// Measures scrolling the wheel over what the pointer rests on.
#[test]
#[ignore = "manual CPU benchmark: cargo test -p pandemonium benchmark_interactions -- --ignored --nocapture --test-threads=1"]
fn benchmark_interactions_scroll() {
    let mut rig = heavy();
    let (x, top) = rig.center(Target::Window(WindowEvent::SidebarSash));
    rig.move_to((x + 300.0, top));
    let mut scroll = Samples::default();
    for step in 0..FRAMES {
        let down = if step < FRAMES / 2 { 40.0 } else { -40.0 };
        scroll.measure(&mut rig, |rig| rig.wheel(down));
    }
    scroll.report("scroll under pointer");
}

/// Measures typing into the prompt, a character a frame, on the heavy
/// workspace and with a long prompt, a long conversation and both.
#[test]
#[ignore = "manual CPU benchmark: cargo test -p pandemonium benchmark_interactions -- --ignored --nocapture --test-threads=1"]
fn benchmark_interactions_typing() {
    let cases: [(&str, pm_pages::WorkspaceState); 4] = [
        ("type in the prompt", crate::snapshot::heavy_workspace()),
        (
            "type in a long prompt",
            crate::snapshot::input_stress(true, false),
        ),
        (
            "type beside a long chat",
            crate::snapshot::input_stress(false, true),
        ),
        (
            "type long prompt+chat",
            crate::snapshot::input_stress(true, true),
        ),
    ];
    for (name, state) in cases {
        let mut rig = Rig::new(Screen::Workspace(state), SIZE);
        rig.click_on(Target::Composer(ComposerEvent::Prompt));
        let mut typing = Samples::default();
        for _ in 0..FRAMES {
            typing.measure(&mut rig, |rig| rig.type_text("x"));
        }
        typing.report(name);
    }
}

/// Measures an agent streaming its reply, a chunk a frame, on the heavy
/// workspace, a long conversation and a long session of tool steps.
#[test]
#[ignore = "manual CPU benchmark: cargo test -p pandemonium benchmark_interactions -- --ignored --nocapture --test-threads=1"]
fn benchmark_interactions_streaming() {
    let cases: [(&str, pm_pages::WorkspaceState); 3] = [
        ("agent streaming", crate::snapshot::heavy_workspace()),
        (
            "streaming into long chat",
            crate::snapshot::input_stress(false, true),
        ),
        (
            "streaming into long session",
            crate::snapshot::long_session(),
        ),
    ];
    for (name, state) in cases {
        let mut rig = Rig::new(Screen::Workspace(state), SIZE);
        let mut streaming = Samples::default();
        for chunk in 0..FRAMES {
            streaming.measure(&mut rig, |rig| {
                rig.hear(pm_harness::Event::Said(format!(
                    "chunk {chunk} of **streamed** words and `code`. "
                )));
            });
        }
        streaming.report(name);
    }
}

/// Scrolls the wheel over the middle of `rig`'s window: `frames` frames of
/// `step` pixels down, then as many back up, measuring each, and reports
/// under `name` with how many frames missed a 120 Hz display's 8.3 ms.
fn scroll_through(rig: &mut Rig, name: &str, step: f32, frames: usize) {
    rig.move_to((SIZE.0 / 2.0, SIZE.1 / 2.0));
    let mut samples = Samples::default();
    for frame in 0..2 * frames {
        let down = if frame < frames { step } else { -step };
        samples.measure(rig, |rig| rig.wheel(down));
    }
    let missed = samples
        .total
        .iter()
        .filter(|ms| **ms > 1000.0 / 120.0)
        .count();
    eprintln!(
        "{name:<28} missed 120 Hz on {missed} of {} frames",
        samples.total.len()
    );
    samples.report(name);
}

/// A view to scroll: its name and the workspace that shows it.
type View = (&'static str, fn() -> pm_pages::WorkspaceState);

/// Scrolls each large view alone in the window, as a trackpad does in small
/// steps and as a flick does in large ones: a 300-message session, a diff
/// of 300 files and one of five thousand lines, a twenty-thousand-line file
/// and an expanded tree of ten thousand rows.
#[test]
#[ignore = "manual CPU benchmark: cargo test -p pandemonium benchmark_interactions -- --ignored --nocapture --test-threads=1"]
fn benchmark_interactions_scroll_views() {
    let views: [View; 5] = [
        ("long session", crate::snapshot::long_session),
        ("large diff", crate::snapshot::large_diff),
        ("large code", crate::snapshot::large_code),
        ("large tree", crate::snapshot::large_tree),
        ("heavy workspace", crate::snapshot::heavy_workspace),
    ];
    for (view, state) in views {
        let mut rig = Rig::new(Screen::Workspace(state()), SIZE);
        scroll_through(&mut rig, &format!("{view} trackpad"), 8.0, 300);
        scroll_through(&mut rig, &format!("{view} flick"), 400.0, 100);
    }
}
