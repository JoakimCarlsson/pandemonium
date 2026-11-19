//! Optional bounded frame measurements.

use std::time::{Duration, Instant};

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

    /// Adds one frame rustle drew, which took `timing` to build and to submit, and
    /// periodically reports percentile timings.
    pub(super) fn frame(&mut self, timing: rustle::FrameTiming) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        self.scene.push(milliseconds(timing.scene));
        self.render.push(milliseconds(timing.render));
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
}
