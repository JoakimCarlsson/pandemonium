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
}

impl Task {
    /// A task called `name` that runs `command` in the root of the project, in `group`, for `language`, found in `source`.
    pub(crate) fn new(
        name: impl Into<String>,
        command: impl Into<String>,
        group: &str,
        language: &str,
        source: &str,
    ) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            folder: String::new(),
            env: BTreeMap::new(),
            group: group.to_owned(),
            language: language.to_owned(),
            source: source.to_owned(),
        }
    }

    /// Whether `name` is what the task is called, or the command it runs, ignoring case.
    pub fn is_named(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name.trim())
            || self.command.eq_ignore_ascii_case(name.trim())
    }
}
