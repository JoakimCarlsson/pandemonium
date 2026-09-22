//! Noticing that a press is the second or third one in the same place.
//!
//! A double click is not something the element tree can report: it hands the
//! window one message per press and has no memory between them. The window
//! has both, so this is where a second press on the same row, tab or
//! character becomes a gesture of its own, and a third one another.

use std::time::{Duration, Instant};

/// Longest interval between two presses that still counts as one gesture.
const INTERVAL: Duration = Duration::from_millis(500);

/// How many presses in a row a gesture counts before starting over.
const LONGEST: usize = 3;

/// The last press on something, for telling a second one from a first.
///
/// `T` is whatever the press landed on — a tab, a row of the file tree, a
/// place in a buffer — and two presses are one gesture when they landed on
/// the same one soon enough.
pub struct Clicks<T> {
    /// What was pressed last, when, and how many times in a row.
    last: Option<(Instant, T, usize)>,
}

impl<T> Default for Clicks<T> {
    /// No press has happened yet.
    fn default() -> Self {
        Self { last: None }
    }
}

impl<T: Copy + PartialEq> Clicks<T> {
    /// Records a press on `what`, saying which one in a row it was.
    ///
    /// The count starts over once the longest gesture has been made, so four
    /// presses in a row are one triple click and one press rather than a
    /// gesture nobody has a name for.
    pub fn press(&mut self, what: T) -> usize {
        let now = Instant::now();
        let count = match self.last {
            Some((at, last, count))
                if last == what && now.duration_since(at) <= INTERVAL && count < LONGEST =>
            {
                count + 1
            }
            _ => 1,
        };
        self.last = Some((now, what, count));
        count
    }

    /// Forgets the last press, for a gesture that was not a press at all.
    pub fn clear(&mut self) {
        self.last = None;
    }
}
