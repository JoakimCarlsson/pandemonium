//! Noticing that a press is the second one in the same place.
//!
//! A double click is not something the element tree can report: it hands the
//! window one message per press and has no memory between them. The window
//! has both, so this is where a second press on the same row, tab or
//! character becomes a gesture of its own.

use std::time::{Duration, Instant};

/// Longest interval between two presses that still counts as one gesture.
const INTERVAL: Duration = Duration::from_millis(500);

/// The last press on something, for telling a second one from a first.
///
/// `T` is whatever the press landed on — a tab, a row of the file tree, a
/// place in a buffer — and two presses are one gesture when they landed on
/// the same one soon enough.
pub struct DoubleClicks<T> {
    /// What was pressed last, and when.
    last: Option<(Instant, T)>,
}

impl<T> Default for DoubleClicks<T> {
    /// No press has happened yet.
    fn default() -> Self {
        Self { last: None }
    }
}

impl<T: Copy + PartialEq> DoubleClicks<T> {
    /// Records a press on `what`, saying whether it was the second one.
    ///
    /// A second press is not itself remembered, so three presses in a row
    /// are one double click and one press rather than two double clicks.
    pub fn press(&mut self, what: T) -> bool {
        let now = Instant::now();
        let twice = self
            .last
            .is_some_and(|(at, last)| last == what && now.duration_since(at) <= INTERVAL);
        self.last = (!twice).then_some((now, what));
        twice
    }

    /// Forgets the last press, for a gesture that was not a press at all.
    pub fn clear(&mut self) {
        self.last = None;
    }
}
