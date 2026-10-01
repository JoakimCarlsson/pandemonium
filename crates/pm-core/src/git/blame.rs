//! Who last changed each line of a file, as git tells it.
//!
//! The porcelain format is asked for because it is the one git promises not
//! to change: one header line per line of the file, followed by whichever
//! fields of the commit git has not already sent. A commit is described
//! once, so the fields are carried forward from the last time they were
//! seen — which is what the `seen` map is for.

use pm_host::Location;

use pm_host::Stdio;
use std::collections::HashMap;
use std::path::Path;

use crate::git::run::holding;

/// How much of a commit hash names it.
const SHORT_HASH: usize = 8;

/// What is said about one line of a file.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Blame {
    /// The commit that last changed it, shortened.
    pub commit: String,
    /// Who wrote that commit.
    pub author: String,
    /// When they wrote it, as a date.
    pub when: String,
    /// What they said they were doing.
    pub summary: String,
    /// Whether the line has never been committed at all.
    pub uncommitted: bool,
}

/// Who last changed each line of `path`, in the repository holding it at or
/// below `root`.
///
/// The lines come back in the order they are in the file, so the nth entry
/// is what to say about the nth line. A file git will not blame — one that
/// is not tracked, or a git that is not there — comes back empty.
pub fn blame(root: impl Into<Location>, path: &Path) -> Vec<Blame> {
    let root = root.into();
    let root = holding(&root, path);
    let Ok(relative) = path.strip_prefix(&root) else {
        return Vec::new();
    };
    let Ok(output) = root
        .host
        .command("git")
        .args(["blame", "--porcelain", "--"])
        .arg(relative)
        .current_dir(&root)
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    read(&String::from_utf8_lossy(&output.stdout))
}

/// The blame of each line, read out of the porcelain format.
fn read(porcelain: &str) -> Vec<Blame> {
    let mut seen: HashMap<String, Blame> = HashMap::new();
    let mut lines = Vec::new();
    let mut current: Option<String> = None;

    for line in porcelain.lines() {
        if let Some(rest) = line.strip_prefix('\t') {
            let _ = rest;
            if let Some(commit) = current.take() {
                lines.push(seen.get(&commit).cloned().unwrap_or_default());
            }
            continue;
        }

        let (field, value) = line.split_once(' ').unwrap_or((line, ""));
        if field.len() == 40 && field.chars().all(|ch| ch.is_ascii_hexdigit()) {
            let commit = field.to_owned();
            seen.entry(commit.clone()).or_insert_with(|| Blame {
                commit: field[..SHORT_HASH].to_owned(),
                uncommitted: field.chars().all(|ch| ch == '0'),
                ..Blame::default()
            });
            current = Some(commit);
            continue;
        }

        let Some(entry) = current.as_ref().and_then(|commit| seen.get_mut(commit)) else {
            continue;
        };
        match field {
            "author" => entry.author = value.to_owned(),
            "author-time" => entry.when = date(value),
            "summary" => entry.summary = value.to_owned(),
            _ => {}
        }
    }
    lines
}

/// A unix timestamp written as a date.
///
/// The date is worked out from the epoch rather than asked of a calendar
/// library: a blame column says which day a line is from, and the arithmetic
/// for that is the civil-from-days one every such library starts with.
fn date(timestamp: &str) -> String {
    let Ok(seconds) = timestamp.parse::<i64>() else {
        return String::new();
    };
    let days = seconds.div_euclid(86_400);
    let (year, month, day) = civil(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The year, month and day `days` after the first of January 1970.
fn civil(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;

    (year + i64::from(month <= 2), month, day)
}
