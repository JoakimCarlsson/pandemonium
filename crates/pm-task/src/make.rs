//! The tasks of a Makefile: its targets.

use std::path::Path;

use crate::task::Task;

/// The names a Makefile is kept under.
const NAMES: [&str; 3] = ["GNUmakefile", "makefile", "Makefile"];

/// The group a target called `name` belongs to.
fn group_of(name: &str) -> &'static str {
    match name {
        "all" | "build" | "release" => "build",
        "test" | "check" | "lint" => "test",
        "run" | "start" | "dev" => "run",
        _ => "",
    }
}

/// Whether `name` is a target a person runs: made of plain characters and not one of make's own.
fn is_runnable(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || "_-./".contains(character))
}

/// The targets the rule on `line` makes, when it is a rule and not a variable or a command.
fn targets_of(line: &str) -> Vec<&str> {
    if line.starts_with(['\t', ' ', '#']) {
        return Vec::new();
    }
    let Some((targets, after)) = line.split_once(':') else {
        return Vec::new();
    };
    if after.starts_with('=') || targets.contains(['=', '$', '%']) {
        return Vec::new();
    }
    targets
        .split_whitespace()
        .filter(|target| is_runnable(target))
        .collect()
}

/// The tasks of the Makefile in `root`, one for each of its targets, or none when it has none.
pub(crate) fn detect(root: &Path) -> Vec<Task> {
    let Some(text) = NAMES
        .iter()
        .find_map(|name| std::fs::read_to_string(root.join(name)).ok())
    else {
        return Vec::new();
    };
    let mut seen: Vec<&str> = Vec::new();
    for target in text.lines().flat_map(targets_of) {
        if !seen.contains(&target) {
            seen.push(target);
        }
    }
    seen.into_iter()
        .map(|target| {
            Task::new(
                format!("make: {target}"),
                format!("make {target}"),
                group_of(target),
                "",
                "Makefile",
            )
        })
        .collect()
}
