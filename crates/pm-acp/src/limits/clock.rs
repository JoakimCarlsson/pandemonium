//! When a window resets, read from however an agent writes a moment down.
//!
//! One agent counts seconds since the epoch, one milliseconds, one writes a
//! date and time as text and one a bare date. All of them come out of here as
//! the same [`SystemTime`].

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

/// The count past which a number since the epoch is taken as milliseconds
/// rather than seconds: in seconds it would be thousands of years away.
const MILLISECONDS_FROM: u64 = 100_000_000_000;

/// The moment `value` names: a count since the epoch, as a number or as the
/// digits of one, or a date and time as text.
pub(super) fn moment(value: &Value) -> Option<SystemTime> {
    match value {
        Value::Number(number) => since_epoch(number.as_f64()?),
        Value::String(text) if text.bytes().all(|byte| byte.is_ascii_digit()) => {
            since_epoch(text.parse().ok()?)
        }
        Value::String(text) => written(text),
        _ => None,
    }
}

/// The moment `count` seconds, or milliseconds, after the epoch.
fn since_epoch(count: f64) -> Option<SystemTime> {
    if !count.is_finite() || count <= 0.0 {
        return None;
    }
    let seconds = match count as u64 >= MILLISECONDS_FROM {
        true => count / 1000.0,
        false => count,
    };
    UNIX_EPOCH.checked_add(Duration::from_secs_f64(seconds))
}

/// The moment `text` writes down, as `2026-10-04T10:03:19.17+00:00`, as
/// `2026-10-04T10:03:19Z` or as the bare date `2026-10-04`, taken as its
/// midnight in UTC.
fn written(text: &str) -> Option<SystemTime> {
    let (date, time) = text.split_once(['T', ' ']).unwrap_or((text, ""));
    let mut parts = date.splitn(3, '-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    let (clock, offset) = offset(time)?;
    let mut fields = clock.splitn(3, ':');
    let hours = fields
        .next()
        .filter(|field| !field.is_empty())
        .map_or(Some(0), |field| field.parse::<i64>().ok())?;
    let minutes = fields
        .next()
        .map_or(Some(0), |field| field.parse::<i64>().ok())?;
    let seconds = fields
        .next()
        .map_or(Some(0.0), |field| field.parse::<f64>().ok())?;
    let whole = days(year, month, day)? * 86_400 + hours * 3600 + minutes * 60 - offset;
    let seconds = whole as f64 + seconds;
    (seconds > 0.0).then(|| UNIX_EPOCH + Duration::from_secs_f64(seconds))
}

/// The time of day in `time`, and how many seconds ahead of UTC it is.
fn offset(time: &str) -> Option<(&str, i64)> {
    if let Some(clock) = time.strip_suffix(['Z', 'z']) {
        return Some((clock, 0));
    }
    let Some(at) = time.rfind(['+', '-']) else {
        return Some((time, 0));
    };
    let (clock, zone) = time.split_at(at);
    let sign = if zone.starts_with('-') { -1 } else { 1 };
    let (hours, minutes) = zone[1..].split_once(':').unwrap_or((&zone[1..], "0"));
    let ahead = hours.parse::<i64>().ok()? * 3600 + minutes.parse::<i64>().ok()? * 60;
    Some((clock, sign * ahead))
}

/// How many days `year`-`month`-`day` falls after the epoch, by the civil
/// calendar.
fn days(year: i64, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year.rem_euclid(400);
    let shifted = i64::from((month + 9) % 12);
    let of_year = (153 * shifted + 2) / 5 + i64::from(day) - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    Some(era * 146_097 + of_cycle - 719_468)
}
