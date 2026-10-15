//! How long ago something happened: the short label a list shows and the part of the day it falls in.

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Datelike, Local};

/// Milliseconds in a minute.
const MINUTE: u64 = 60_000;

/// Milliseconds in an hour.
const HOUR: u64 = 60 * MINUTE;

/// Milliseconds in a day.
const DAY: u64 = 24 * HOUR;

/// The stretch of time a session last changed in, newest first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Span {
    /// On the current calendar day.
    Today,
    /// On the calendar day before that.
    Yesterday,
    /// Longer ago.
    Earlier,
}

impl Span {
    /// Every span, newest first.
    pub const ALL: [Span; 3] = [Span::Today, Span::Yesterday, Span::Earlier];

    /// The span a change at `then` falls in when it is `now`, by local calendar day.
    pub fn of(then: u64, now: u64) -> Self {
        match local_day(now).saturating_sub(local_day(then)) {
            0 => Self::Today,
            1 => Self::Yesterday,
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

/// The number of the local calendar day `millis` since the epoch falls on.
fn local_day(millis: u64) -> i64 {
    DateTime::from_timestamp_millis(millis as i64).map_or(0, |moment| {
        moment.with_timezone(&Local).date_naive().num_days_from_ce() as i64
    })
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
