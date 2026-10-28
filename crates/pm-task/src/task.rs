//! The value a task is.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One thing a project can be told to do: build, test, run, or whatever its own file names.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// What it is called; a debug scenario names the task it needs first by this.
    pub name: String,
    /// What is typed into the shell to run it.
    pub command: String,
    /// The folder it runs in, from the root of the project; empty for the root itself.
    #[serde(default)]
    pub folder: String,
    /// What the environment holds besides what the shell starts with.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// What kind of task it is: `build`, `test`, `run`, or nothing.
    #[serde(default)]
    pub group: String,
    /// The language whose toolchain it runs with, by the language's folder name, or nothing when it has none.
    #[serde(default)]
    pub language: String,
    /// Where it was found: the manifest it was read from.
    #[serde(default)]
    pub source: String,
    /// The tags of the places it is run from: a place of a file that a language tags the same is run by it, and a task of the project with the tag is run in place of a language's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Task {
    /// Whether `name` is what the task is called, or the command it runs, ignoring case.
    pub fn is_named(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name.trim())
            || self.command.eq_ignore_ascii_case(name.trim())
    }
}
