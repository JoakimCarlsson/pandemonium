//! Commands a worktree offers from its own files.

use pm_host::Location;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::repositories;

/// The file containing tasks defined by a project.
const TASKS_FILE: &str = ".pandemonium/tasks.yaml";

/// Where a task came from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskSource {
    /// The project's tasks file.
    File,
    /// A file detected in the worktree.
    Detected(&'static str),
}

/// A command a worktree can run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    /// Its name in the picker and in a run.
    pub label: String,
    /// The command line handed to the user's shell.
    pub command: String,
    /// Its working directory relative to the worktree root.
    pub cwd: Option<PathBuf>,
    /// Variables added to the run's environment.
    pub env: Vec<(String, String)>,
    /// The file that defined or implied it.
    pub source: TaskSource,
    /// Whether this task contributes to worktree health.
    pub check: bool,
}

/// The YAML representation of one project task.
#[derive(Deserialize)]
struct WrittenTask {
    /// Whether this command is a health check.
    #[serde(default)]
    check: bool,
    /// Its label.
    label: String,
    /// Its shell command.
    command: String,
    /// Its working directory.
    cwd: Option<PathBuf>,
    /// Its additional environment.
    #[serde(default)]
    env: std::collections::BTreeMap<String, String>,
}

/// All tasks the worktree at `root` offers.
pub fn tasks(root: impl Into<Location>) -> Vec<Task> {
    tasks_checked(root).0
}

/// All tasks and any error reading the project's tasks file.
pub fn tasks_checked(root: impl Into<Location>) -> (Vec<Task>, Option<String>) {
    let root = root.into();
    let file = root.join(TASKS_FILE);
    let (mut found, error) = match root.host.fs().read_to_string(&file) {
        Ok(content) => match serde_norway::from_str::<Vec<WrittenTask>>(&content) {
            Ok(written) => (
                written
                    .into_iter()
                    .map(|task| Task {
                        label: task.label,
                        command: task.command,
                        cwd: task.cwd,
                        env: task.env.into_iter().collect(),
                        source: TaskSource::File,
                        check: task.check,
                    })
                    .collect::<Vec<_>>(),
                None,
            ),
            Err(error) => (
                Vec::new(),
                Some(format!("Could not read {TASKS_FILE}: {error}")),
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Vec::new(), None),
        Err(error) => (
            Vec::new(),
            Some(format!("Could not read {TASKS_FILE}: {error}")),
        ),
    };
    let mut labels = found
        .iter()
        .map(|task| task.label.clone())
        .collect::<HashSet<_>>();
    let mut roots = vec![root.to_path_buf()];
    roots.extend(
        repositories(&root)
            .into_iter()
            .map(|location| location.path)
            .filter(|path| path != &root.path),
    );
    for directory in roots {
        let relative = directory.strip_prefix(&root).unwrap_or(Path::new(""));
        for task in detected(&root.at(&directory)) {
            let label = if relative.as_os_str().is_empty() {
                task.label.clone()
            } else {
                format!("{}: {}", relative.display(), task.label)
            };
            if labels.insert(label.clone()) {
                found.push(Task {
                    label,
                    cwd: (!relative.as_os_str().is_empty()).then(|| relative.to_path_buf()),
                    ..task
                });
            }
        }
    }
    (found, error)
}

/// Tasks inferred from manifests and make targets in `root`.
fn detected(root: &Location) -> Vec<Task> {
    let mut found = Vec::new();
    let mut add = |label: &str, command: String, source| {
        found.push(Task {
            label: label.to_owned(),
            command,
            cwd: None,
            env: Vec::new(),
            source: TaskSource::Detected(source),
            check: false,
        })
    };
    if let Ok(cargo) = root.host.fs().read_to_string(root.join("Cargo.toml")) {
        for name in ["build", "test", "check", "clippy"] {
            add(name, format!("cargo {name}"), "cargo");
        }
        if cargo.lines().any(|line| line.trim() == "[[bin]]")
            || root.host.fs().is_file(root.join("src/main.rs"))
        {
            add("run", "cargo run".to_owned(), "cargo");
        }
    }
    if let Ok(package) = root.host.fs().read_to_string(root.join("package.json"))
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&package)
        && let Some(scripts) = value.get("scripts").and_then(serde_json::Value::as_object)
    {
        let manager = if root.host.fs().exists(root.join("pnpm-lock.yaml")) {
            "pnpm"
        } else if root.host.fs().exists(root.join("yarn.lock")) {
            "yarn"
        } else if root.host.fs().exists(root.join("bun.lock"))
            || root.host.fs().exists(root.join("bun.lockb"))
        {
            "bun"
        } else {
            "npm"
        };
        for name in scripts.keys() {
            add(name, format!("{manager} run {name}"), "npm");
        }
    }
    for filename in ["Makefile", "makefile", "GNUmakefile"] {
        if let Ok(content) = root.host.fs().read_to_string(root.join(filename)) {
            for line in content.lines() {
                if let Some((target, _)) = line.split_once(':')
                    && !target.is_empty()
                    && !target.starts_with('.')
                    && !target.contains(['%', '$', '='])
                    && !target.chars().any(char::is_whitespace)
                {
                    add(target, format!("make {target}"), "make");
                }
            }
            break;
        }
    }
    if root.host.fs().is_file(root.join("go.mod")) {
        for (name, command) in [
            ("build", "go build ./..."),
            ("test", "go test ./..."),
            ("vet", "go vet ./..."),
        ] {
            add(name, command.to_owned(), "go");
        }
    }
    found
}
