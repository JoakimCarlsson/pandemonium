//! Optional bounded frame measurements and redraw suppression for unchanged tree hovers.

use std::time::{Duration, Instant};

use pm_input::PointerState;
use pm_pages::Screen;
use pm_ui::{Output, SCROLLBAR_GRIP};

use super::TREE_SCROLL;

/// How many presented frames are summarized at once.
const SAMPLES: usize = 240;

/// CPU-side timings, enabled by `PANDEMONIUM_FRAME_TIMINGS=1`.
pub(super) struct Timings {
    /// Whether measurement was requested at startup.
    enabled: bool,
    /// The oldest pointer event awaiting a successfully submitted frame.
    pointer: Option<Instant>,
    /// Scene construction, layout and painting times, in milliseconds.
    scene: Vec<f64>,
    /// Surface acquisition, encoding and submission times, in milliseconds.
    render: Vec<f64>,
    /// Pointer event to successful render return, in milliseconds.
    response: Vec<f64>,
    /// The oldest key or IME event awaiting a submitted frame.
    keyboard: Option<Instant>,
    /// Keyboard handling through successful submission, in milliseconds.
    key_response: Vec<f64>,
}

impl Default for Timings {
    /// Reads the opt-in switch once; ordinary frames allocate no measurement buffers.
    fn default() -> Self {
        Self {
            enabled: std::env::var("PANDEMONIUM_FRAME_TIMINGS").is_ok_and(|value| value == "1"),
            pointer: None,
            scene: Vec::new(),
            render: Vec::new(),
            response: Vec::new(),
            keyboard: None,
            key_response: Vec::new(),
        }
    }
}

impl Timings {
    /// Records the oldest pointer change waiting for a redraw.
    pub(super) fn pointer(&mut self, received: Option<Instant>) {
        if self.enabled && self.pointer.is_none() {
            self.pointer = received;
        }
    }

    /// Starts measuring a key or composition event before its edit is applied.
    pub(super) fn keyboard(&mut self) {
        if self.enabled && self.keyboard.is_none() {
            self.keyboard = Some(Instant::now());
        }
    }

    /// Starts a CPU timing when measurement is enabled.
    pub(super) fn start(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    /// Adds one successful frame and periodically reports percentile timings.
    pub(super) fn frame(
        &mut self,
        started: Option<Instant>,
        painted: Option<Instant>,
        presented: bool,
    ) {
        let (Some(started), Some(painted)) = (started, painted) else {
            return;
        };
        if !presented {
            return;
        }
        let now = Instant::now();
        self.scene
            .push(milliseconds(painted.duration_since(started)));
        self.render.push(milliseconds(now.duration_since(painted)));
        if let Some(pointer) = self.pointer.take() {
            self.response
                .push(milliseconds(now.duration_since(pointer)));
        }
        if let Some(keyboard) = self.keyboard.take() {
            self.key_response
                .push(milliseconds(now.duration_since(keyboard)));
        }
        if self.scene.len() == SAMPLES {
            eprintln!(
                "pandemonium frame timings ({SAMPLES} frames; milliseconds p50/p95/p99): scene={} acquire+submit={} pointer-to-submit={} ({} pointer frames) key-to-submit={} ({} key frames; excludes display latency)",
                percentiles(&mut self.scene),
                percentiles(&mut self.render),
                percentiles(&mut self.response),
                self.response.len(),
                percentiles(&mut self.key_response),
                self.key_response.len()
            );
            self.scene.clear();
            self.render.clear();
            self.response.clear();
            self.key_response.clear();
        }
    }
}

/// Converts a measured duration into fractional milliseconds.
fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Formats nearest-rank percentiles without retaining data beyond this batch.
fn percentiles(samples: &mut [f64]) -> String {
    if samples.is_empty() {
        return "n/a".to_owned();
    }
    samples.sort_unstable_by(f64::total_cmp);
    let at = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1];
    format!("{:.2}/{:.2}/{:.2}", at(50), at(95), at(99))
}

/// Whether moving within a tree row leaves both its highlight and scrollbar unchanged.
pub(super) fn same_tree_hover(
    output: &Output,
    screen: &Screen,
    before: PointerState,
    after: PointerState,
) -> bool {
    if before.left
        || before.right
        || before.middle
        || after.left
        || after.right
        || after.middle
        || output.animating
    {
        return false;
    }
    let (Some(before), Some(after)) = (before.position, after.position) else {
        return false;
    };
    let Some(id) = output
        .hit(before)
        .filter(|id| Some(*id) == output.hit(after))
    else {
        return false;
    };
    if screen.tree_row_of(id).is_none_or(|target| target.is_root()) {
        return false;
    }
    let Some(scroller) = output
        .scrollers
        .iter()
        .find(|scroller| scroller.id == TREE_SCROLL)
    else {
        return false;
    };
    let grip = |point: (f32, f32)| {
        scroller.rect.contains(point.0, point.1)
            && point.0 >= scroller.rect.x + scroller.rect.width - SCROLLBAR_GRIP
    };
    grip(before) == grip(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Percentiles include the slow tail, and empty pointer batches remain explicit.
    #[test]
    fn reports_tail_and_empty_batches() {
        assert_eq!(percentiles(&mut []), "n/a");
        let mut samples: Vec<f64> = (1..=100).rev().map(f64::from).collect();
        assert_eq!(percentiles(&mut samples), "50.00/95.00/99.00");
    }

    /// Only unchanged, unpressed tree rows suppress redraws; scrollbar transitions still repaint.
    #[test]
    fn tree_hover_redraw_boundaries() {
        use pm_components::state::WorkspaceState;
        use pm_components::tree_actions::TREE_FILE_BASE;
        use pm_input::FocusId;
        use pm_renderer::Rect;
        use pm_ui::{Region, Scroller};

        let mut state = WorkspaceState::sample();
        state.files = vec!["a.rs".to_owned(), "b.rs".to_owned()];
        let screen = Screen::Workspace(state);
        let mut output = Output {
            regions: vec![
                Region {
                    id: FocusId(TREE_FILE_BASE),
                    rect: Rect::new(0.0, 0.0, 200.0, 24.0),
                },
                Region {
                    id: FocusId(TREE_FILE_BASE + 1),
                    rect: Rect::new(0.0, 24.0, 200.0, 24.0),
                },
            ],
            scrollers: vec![Scroller {
                id: TREE_SCROLL,
                rect: Rect::new(0.0, 0.0, 200.0, 100.0),
                room: 100.0,
                offset: 0.0,
            }],
            ..Output::default()
        };
        let pointer = |x, y| PointerState {
            position: Some((x, y)),
            ..PointerState::default()
        };
        let before = pointer(20.0, 10.0);
        assert!(same_tree_hover(
            &output,
            &screen,
            before,
            pointer(40.0, 15.0)
        ));
        assert!(!same_tree_hover(
            &output,
            &screen,
            before,
            pointer(40.0, 30.0)
        ));
        assert!(!same_tree_hover(
            &output,
            &screen,
            before,
            pointer(195.0, 10.0)
        ));
        assert!(!same_tree_hover(
            &output,
            &screen,
            before,
            PointerState {
                left: true,
                ..before
            }
        ));
        output.animating = true;
        assert!(!same_tree_hover(
            &output,
            &screen,
            before,
            pointer(40.0, 15.0)
        ));
    }
}
