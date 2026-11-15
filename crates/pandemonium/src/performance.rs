//! Frame measurements, switched on with `PANDEMONIUM_FRAME_TIMINGS=1`: every few hundred frames the window writes how long building and drawing them took to stderr.

use std::cell::RefCell;
use std::time::Duration;

use rustle::FrameTiming;

/// How many submitted frames are summed up at once.
const SAMPLES: usize = 240;

/// The environment variable that switches the measurements on when it is `1`.
const SWITCH: &str = "PANDEMONIUM_FRAME_TIMINGS";

/// Starts measuring the window's frames when the environment asks for it; ordinary runs measure nothing.
pub fn install() {
    if std::env::var(SWITCH).is_ok_and(|value| value == "1") {
        let samples = RefCell::new(Vec::with_capacity(SAMPLES));
        rustle::set_frame_observer(move |timing| record(&samples, timing));
    }
}

/// Adds `timing` to `samples`, and once they are full writes their summary to stderr and starts over.
fn record(samples: &RefCell<Vec<FrameTiming>>, timing: FrameTiming) {
    let mut samples = samples.borrow_mut();
    samples.push(timing);
    if samples.len() < SAMPLES {
        return;
    }
    let scene: Vec<Duration> = samples.iter().map(|timing| timing.scene).collect();
    let render: Vec<Duration> = samples.iter().map(|timing| timing.render).collect();
    eprintln!(
        "pandemonium frames ({SAMPLES}): scene {} · render {}",
        summary(scene),
        summary(render)
    );
    samples.clear();
}

/// The median, 95th and 99th percentiles of `times`, in milliseconds.
fn summary(mut times: Vec<Duration>) -> String {
    times.sort();
    let at = |share: f64| {
        let index = ((times.len() - 1) as f64 * share).round() as usize;
        times[index].as_secs_f64() * 1000.0
    };
    format!(
        "p50 {:.2} ms, p95 {:.2} ms, p99 {:.2} ms",
        at(0.50),
        at(0.95),
        at(0.99)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The summary picks the right frames from the sorted times.
    #[test]
    fn summarises_percentiles() {
        let times = (1..=100).rev().map(Duration::from_millis).collect();
        assert_eq!(summary(times), "p50 51.00 ms, p95 95.00 ms, p99 99.00 ms");
    }
}
