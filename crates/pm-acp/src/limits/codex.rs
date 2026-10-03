//! Codex's limits, as Codex writes them into its own session files.
//!
//! Its adapter passes none of them on, but Codex itself writes the rate
//! limits it was last told of beside each token count it records under
//! `sessions/` in its home. The windows are the account's, not the
//! conversation's, so the newest record in the newest file is the one read.
//! The format is Codex's own and may change under it.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

use super::{Limits, Window, clock};

/// The minutes in a day and in a week, which name the windows that span them.
const DAY: u64 = 24 * 60;

/// The minutes in a week.
const WEEK: u64 = 7 * DAY;

/// The limits Codex last recorded, where it has recorded any.
pub(super) fn read() -> Option<Limits> {
    read_at(&home()?)
}

/// The latest account limits recorded under this session's selected Codex home.
pub(super) fn read_at(home: &Path) -> Option<Limits> {
    let day = newest_day(&home.join("sessions"))?;
    let mut files = fs::read_dir(day)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
        .map(|entry| (modified(&entry.path()), entry.path()))
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    files.iter().find_map(|(_, file)| recorded(file))
}

/// Where Codex keeps its files: `CODEX_HOME`, or `.codex` in the home.
fn home() -> Option<PathBuf> {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(env::home_dir()?.join(".codex")))
}

/// The newest of the year, month and day directories Codex files its
/// sessions under, whose names sort as the dates they are.
fn newest_day(sessions: &Path) -> Option<PathBuf> {
    (0..3).try_fold(sessions.to_path_buf(), |directory, _| {
        fs::read_dir(&directory)
            .ok()?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.path())
            .max()
    })
}

/// When `file` was last written.
fn modified(file: &Path) -> Option<SystemTime> {
    fs::metadata(file).ok()?.modified().ok()
}

/// The last rate limits recorded in `file` that name a window.
fn recorded(file: &Path) -> Option<Limits> {
    let records = fs::read_to_string(file).ok()?;
    records
        .lines()
        .rev()
        .filter(|line| line.contains("\"rate_limits\""))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|record| limits(&record["payload"]["rate_limits"]))
}

/// The limits one record's `rate_limits` comes to, where it names a window.
fn limits(rate_limits: &Value) -> Option<Limits> {
    let windows = ["primary", "secondary"]
        .iter()
        .filter_map(|which| window(&rate_limits[*which]))
        .collect::<Vec<_>>();
    if windows.is_empty() {
        return None;
    }
    Limits::of(rate_limits["plan_type"].as_str(), windows)
}

/// The one window `window` describes.
fn window(window: &Value) -> Option<Window> {
    Some(Window {
        label: span(window["window_minutes"].as_u64()?),
        used: window["used_percent"].as_f64()?,
        resets: clock::moment(&window["resets_at"]),
    })
}

/// What a window of `minutes` is called.
fn span(minutes: u64) -> String {
    match minutes {
        WEEK => "Weekly".to_owned(),
        DAY => "Daily".to_owned(),
        minutes if minutes % 60 == 0 => format!("{}-hour", minutes / 60),
        minutes => format!("{minutes}-minute"),
    }
}
