//! The tasks of a `package.json`: its scripts.

use std::path::Path;

use crate::task::Task;

/// The program that runs the scripts of the project in `root`, by the lockfile it keeps.
fn runner(root: &Path) -> &'static str {
    if root.join("pnpm-lock.yaml").is_file() {
        "pnpm"
    } else if root.join("yarn.lock").is_file() {
        "yarn"
    } else if root.join("bun.lock").is_file() || root.join("bun.lockb").is_file() {
        "bun"
    } else {
        "npm"
    }
}

/// The group a script called `name` belongs to.
fn group_of(name: &str) -> &'static str {
    match name {
        "build" | "compile" => "build",
        "test" | "check" => "test",
        "start" | "dev" | "serve" => "run",
        _ => "",
    }
}

/// The tasks of the `package.json` in `root`, one for each of its scripts, or none when it has none.
pub(crate) fn detect(root: &Path) -> Vec<Task> {
    let Ok(text) = std::fs::read_to_string(root.join("package.json")) else {
        return Vec::new();
    };
    let Ok(package) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(scripts) = package
        .get("scripts")
        .and_then(|scripts| scripts.as_object())
    else {
        return Vec::new();
    };
    let runner = runner(root);
    scripts
        .keys()
        .map(|name| {
            Task::new(
                format!("{runner}: {name}"),
                format!("{runner} run {name}"),
                group_of(name),
                "javascript",
                "package.json",
            )
        })
        .collect()
}
