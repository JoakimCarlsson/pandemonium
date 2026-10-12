//! The retained log and live event stream of one operation.

use crate::event::{Event, Phase, Sink, State};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Writes every event to the environment's log file and forwards it to the caller.
///
/// The file outlives the operation, so a failed build or setup can be read
/// afterwards, and it is only cleared when the next attempt starts.
pub(super) struct Journal {
    /// The caller's live view of the operation.
    sink: Sink,
    /// The log file, absent when it could not be opened.
    file: Mutex<Option<std::fs::File>>,
    /// Where the log is kept.
    path: PathBuf,
    /// The environment's state, shared with the environment handle.
    state: Arc<Mutex<State>>,
}

impl Journal {
    /// Opens the log at `path`, clearing it when `fresh`.
    pub(super) fn start(path: PathBuf, fresh: bool, sink: Sink, state: Arc<Mutex<State>>) -> Self {
        if let Some(parent) = path.parent() {
            let _ = crate::record::private_directory(parent);
        }
        let file = crate::record::private_file(&path, fresh).ok();
        Self {
            sink,
            file: Mutex::new(file),
            path,
            state,
        }
    }

    /// Where the log is kept.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Appends one line to the log file.
    fn write(&self, line: &str) {
        if let Some(file) = self.file.lock().unwrap().as_mut() {
            let _ = writeln!(file, "{line}");
        }
    }

    /// Records and forwards one output line.
    pub(super) fn log(&self, source: &str, line: &str) {
        self.write(&format!("[{source}] {line}"));
        (self.sink)(Event::Log {
            source: source.to_owned(),
            line: line.to_owned(),
        });
    }

    /// Records and forwards a notice that is not a failure.
    pub(super) fn notice(&self, text: impl Into<String>) {
        let text = text.into();
        self.write(&format!("[notice] {text}"));
        (self.sink)(Event::Notice(text));
    }

    /// Moves the environment to a new state.
    pub(super) fn state(&self, state: State) {
        *self.state.lock().unwrap() = state.clone();
        (self.sink)(Event::State(state));
    }

    /// Announces the step now under way.
    pub(super) fn phase(&self, phase: Phase) {
        self.write(&format!("== {}", phase.describe()));
        self.state(State::Working(phase));
    }

    /// Forwards a change of the desktop listeners.
    pub(super) fn forwards(&self, forwards: Vec<crate::event::Forward>) {
        for forward in &forwards {
            self.write(&format!("[forward] {}", forward.describe()));
        }
        (self.sink)(Event::Forwards(forwards));
    }
}
