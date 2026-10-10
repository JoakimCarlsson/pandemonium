//! Python unittest discovery and execution plans handed to the task seam.

use crate::{Task, TaskSource};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A subset of the adapter's discovered cases.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Selection {
    /// Every case discovered by the adapter.
    All,
    /// Every case in a suite.
    Suite(String),
    /// Exactly one framework identity.
    Case(String),
}

/// Project-local unittest discovery configuration.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Python {
    /// Interpreter executable, including a worktree-local virtual environment.
    pub interpreter: String,
    /// Discovery directory relative to the worktree.
    pub start: PathBuf,
    /// Import root relative to the worktree.
    pub top: PathBuf,
    /// unittest filename pattern.
    pub pattern: String,
}

impl Default for Python {
    /// Uses the standard interpreter and unittest discovery defaults.
    fn default() -> Self {
        Self {
            interpreter: "python3".into(),
            start: ".".into(),
            top: ".".into(),
            pattern: "test*.py".into(),
        }
    }
}

impl Python {
    /// Reads optional project configuration and validates its worktree-relative directories.
    pub fn read(root: &Path) -> Result<Self, String> {
        let file = root.join(".pandemonium/tests.json");
        let adapter: Self = match std::fs::read_to_string(file) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => return Err(error.to_string()),
        };
        let resolved_root = root.canonicalize().map_err(|error| error.to_string())?;
        for path in [&adapter.start, &adapter.top] {
            if path.is_absolute()
                || path
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
            {
                return Err("Test directories must be inside their worktree".into());
            }
            let directory = root
                .join(path)
                .canonicalize()
                .map_err(|error| error.to_string())?;
            if !directory.is_dir() || !directory.starts_with(&resolved_root) {
                return Err("Test directories must resolve inside their worktree".into());
            }
        }
        Ok(adapter)
    }

    /// Writes a unique run plan and returns a command for the existing task runner.
    pub fn task(
        &self,
        directory: &Path,
        selection: Option<Selection>,
        coverage: bool,
    ) -> Result<Task, String> {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let script = directory.join("runner.py");
        std::fs::write(&script, include_str!("bridge.py")).map_err(|error| error.to_string())?;
        let plan = directory.join("plan.json");
        let value =
            serde_json::json!({ "adapter": self, "selection": selection, "coverage": coverage });
        std::fs::write(&plan, value.to_string()).map_err(|error| error.to_string())?;
        Ok(Task {
            label: if selection.is_none() {
                "Tests: discover"
            } else {
                "Tests: run"
            }
            .into(),
            command: format!(
                "{} -u {} {}",
                quote(&self.interpreter),
                quote(&script.to_string_lossy()),
                quote(&plan.to_string_lossy())
            ),
            cwd: None,
            env: Vec::new(),
            source: TaskSource::Detected("unittest"),
            check: false,
        })
    }

    /// Arguments for debugging the exact discovered case through debugpy.
    pub fn debug_arguments(
        &self,
        root: &Path,
        id: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        serde_json::json!({ "module": "unittest", "args": [id], "cwd": root.join(&self.top), "python": [if Path::new(&self.interpreter).components().count() > 1 { root.join(&self.interpreter).to_string_lossy().into_owned() } else { self.interpreter.clone() }], "justMyCode": false }).as_object().cloned().unwrap_or_default()
    }
}

/// Quotes one argument for the shell used by project tasks.
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
