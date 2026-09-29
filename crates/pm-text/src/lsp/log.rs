//! What a server said besides its answers, written down to be read later.
//!
//! A server writes its troubles on its error stream and in log messages, and
//! a server that misbehaves says why there and nowhere else. Each server over
//! each worktree writes to a file of its own under the editor's home, which
//! is opened in a pane like any other file; a file rather than a buffer, so
//! what a server said before it died is still there after it has.

use std::fs::{File, OpenOptions};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use lsp_types::MessageType;

/// How large a log may grow before the next start begins it afresh.
const LIMIT: u64 = 4 * 1024 * 1024;

/// Where one server writes what it says, and whether it has said more.
#[derive(Clone)]
pub struct Log {
    /// The file, while it is open.
    file: Arc<Mutex<Option<File>>>,
    /// Where it lives.
    path: Option<PathBuf>,
    /// Whether anything was written since this was last asked.
    grew: Arc<AtomicBool>,
}

impl Log {
    /// The log of `command` over `root`, in `directory`, begun with a line
    /// saying the server is starting.
    ///
    /// Without a directory nothing is written: a server runs the same
    /// whether or not anybody keeps what it says.
    pub(super) fn open(directory: Option<&Path>, root: &Path, command: &str) -> Self {
        let path = directory.map(|directory| directory.join(name(root, command)));
        let file = path.as_deref().and_then(|path| {
            std::fs::create_dir_all(path.parent()?).ok()?;
            let fresh = std::fs::metadata(path).is_ok_and(|found| found.len() > LIMIT);
            OpenOptions::new()
                .create(true)
                .append(!fresh)
                .write(true)
                .truncate(fresh)
                .open(path)
                .ok()
        });
        let log = Self {
            file: Arc::new(Mutex::new(file)),
            path,
            grew: Arc::new(AtomicBool::new(false)),
        };
        log.write(&format!(
            "── {command} starting over {} at {} ──",
            root.display(),
            now()
        ));
        log
    }

    /// Where the log lives, when it is kept.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Writes one line.
    pub(super) fn write(&self, line: &str) {
        let Ok(mut file) = self.file.lock() else {
            return;
        };
        if let Some(file) = file.as_mut()
            && writeln!(file, "{line}").is_ok()
        {
            self.grew.store(true, Ordering::Relaxed);
        }
    }

    /// Writes one message the server logged or showed, marked with its kind.
    pub(super) fn message(&self, kind: MessageType, message: &str) {
        let kind = match kind {
            MessageType::ERROR => "error",
            MessageType::WARNING => "warning",
            MessageType::INFO => "info",
            _ => "log",
        };
        for line in message.lines() {
            self.write(&format!("[{kind}] {line}"));
        }
    }

    /// Copies everything `stream` says into the log until it ends, on a
    /// thread of its own.
    pub(super) fn follow(&self, stream: impl Read + Send + 'static) {
        let log = self.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else {
                    break;
                };
                log.write(&line);
            }
        });
    }

    /// Whether anything was written since this was last asked.
    pub fn take_grown(&self) -> bool {
        self.grew.swap(false, Ordering::Relaxed)
    }
}

/// The name of the log of `command` over `root`: readable, and apart from the
/// log of the same server over another checkout of the same name.
fn name(root: &Path, command: &str) -> String {
    let mut hasher = DefaultHasher::new();
    root.hash(&mut hasher);
    let folder = root
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    format!("{folder}-{command}-{:08x}.log", hasher.finish() as u32)
}

/// The time now, written out in UTC.
fn now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs()) as i64;
    let (year, month, day) = civil(seconds.div_euclid(86_400));
    let time = seconds.rem_euclid(86_400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        time / 3600,
        time % 3600 / 60,
        time % 60
    )
}

/// The year, month and day `days` after the epoch fall on.
fn civil(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let of_era = shifted.rem_euclid(146_097);
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let day_of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}
