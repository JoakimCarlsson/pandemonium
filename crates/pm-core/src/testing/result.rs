//! Framework-independent case identities and results within a worktree.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A source position counted from zero.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Location {
    /// Absolute source path in the originating worktree.
    pub path: PathBuf,
    /// Source line counted from zero.
    pub line: usize,
}

/// The latest state of an individual test.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Discovered but not run.
    #[default]
    Unknown,
    /// Waiting to run.
    Queued,
    /// Executing now.
    Running,
    /// Passed normally.
    Passed,
    /// Assertion, setup or execution failed.
    Failed,
    /// Skipped or an expected failure.
    Skipped,
    /// Interrupted before completion.
    Cancelled,
}

/// A discoverable case and its most recently retained result.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Case {
    /// Framework identity, unique inside one scope.
    pub id: String,
    /// Suite identity used for grouping and selection.
    pub suite: String,
    /// Navigable definition, absent for discovery failures.
    pub location: Option<Location>,
    /// Latest result.
    #[serde(default)]
    pub status: Status,
    /// Wall-clock seconds spent executing the case.
    #[serde(default)]
    pub duration: f64,
    /// Unix timestamp of the case start, used to time interrupted cases.
    #[serde(default)]
    pub started: f64,
    /// Standard output, standard error and failure details.
    #[serde(default)]
    pub output: String,
    /// The innermost failure location inside the originating worktree.
    #[serde(default)]
    pub failure: Option<Location>,
}
