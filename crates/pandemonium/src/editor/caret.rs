//! The clock that makes the caret blink.
//!
//! A caret that never blinks is a caret the reader has to look for; one that
//! blinks while they are typing is one that flickers under their hands. So
//! the blink is a clock that anything the reader does puts back to the
//! beginning: the caret is solid the instant a key is pressed, and starts
//! counting again from there.

use std::time::{Duration, Instant};

/// How long the caret stays solid, and how long it then stays hidden.
const PHASE: Duration = Duration::from_millis(500);

/// Where the caret is in its blink.
#[derive(Clone, Copy, Debug)]
pub struct Blink {
    /// When the caret was last made solid by something happening.
    since: Instant,
    /// Whether it was drawn solid the last time it was drawn.
    shown: bool,
}

impl Default for Blink {
    /// A caret that has just been made solid.
    fn default() -> Self {
        Self {
            since: Instant::now(),
            shown: true,
        }
    }
}

impl Blink {
    /// Makes the caret solid and starts the blink again.
    pub fn restart(&mut self) {
        *self = Self::default();
    }

    /// Whether the caret is solid this instant.
    pub fn is_solid(&self) -> bool {
        (self.since.elapsed().as_millis() / PHASE.as_millis()).is_multiple_of(2)
    }

    /// Takes up the change of state a redraw is owed, if there is one.
    pub fn changed(&mut self) -> bool {
        let solid = self.is_solid();
        let changed = solid != self.shown;
        self.shown = solid;
        changed
    }

    /// When the caret next turns solid or hollow.
    pub fn next_change(&self) -> Instant {
        let elapsed = self.since.elapsed();
        let phases = elapsed.as_millis() / PHASE.as_millis() + 1;
        self.since + PHASE * phases as u32
    }
}
