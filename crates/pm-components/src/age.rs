//! How long ago something happened: the short label a list shows and the part of the day it falls in.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds in a minute.
const MINUTE: u64 = 60_000;

/// Milliseconds in an hour.
const HOUR: u64 = 60 * MINUTE;

/// Milliseconds in a day.
const DAY: u64 = 24 * HOUR;

/// The stretch of time a session last changed in, newest first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Span {
    /// Within the last day.
    Today,
    /// The day before that.
    Yesterday,
    /// Longer ago.
    Earlier,
}

impl Span {
    /// Every span, newest first.
    pub const ALL: [Span; 3] = [Span::Today, Span::Yesterday, Span::Earlier];

    /// The span a change at `then` falls in when it is `now`.
    pub fn of(then: u64, now: u64) -> Self {
        match now.saturating_sub(then) {
            elapsed if elapsed < DAY => Self::Today,
            elapsed if elapsed < 2 * DAY => Self::Yesterday,
            _ => Self::Earlier,
        }
    }

    /// What the heading over the span says.
    pub fn title(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Yesterday => "Yesterday",
            Self::Earlier => "Earlier",
        }
    }
}

/// The time now, in milliseconds since the epoch.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// How long ago `then` was when it is `now`, such as `just now`, `4m ago` or `3d ago`.
pub fn label(then: u64, now: u64) -> String {
    let elapsed = now.saturating_sub(then);
    match elapsed {
        elapsed if elapsed < MINUTE => "just now".to_owned(),
        elapsed if elapsed < HOUR => format!("{}m ago", elapsed / MINUTE),
        elapsed if elapsed < DAY => format!("{}h ago", elapsed / HOUR),
        elapsed => format!("{}d ago", elapsed / DAY),
    }
}
