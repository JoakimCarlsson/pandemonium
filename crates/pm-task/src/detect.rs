//! The tasks of a folder: those the language folders say its files imply and those it writes down.

use std::path::Path;

use regex::Regex;

use crate::file;
use crate::spec::{Each, TaskSpec};
use crate::task::Task;

/// What reading a folder's tasks found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Found {
    /// The tasks, those the project writes first.
    pub tasks: Vec<Task>,
    /// Why the project's own file could not be read, when it could not.
    pub problem: Option<String>,
}

/// Whether `name` is made of the characters a script, a target or a service is named with, so that it is safe to type into a shell.
fn is_plain(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|held| held.is_alphanumeric() || "_-.:/@+".contains(held))
}

/// Whether the pattern `pattern` matches the text of the file `file` in `root`.
fn matches(root: &Path, file: &str, pattern: &str) -> bool {
    let (Ok(text), Ok(pattern)) = (
        std::fs::read_to_string(root.join(file)),
        Regex::new(pattern),
    ) else {
        return false;
    };
    pattern.is_match(&text)
}

/// The file that makes `spec` apply in `root`, or nothing when it does not: its conditions all hold. A spec with no file conditions applies and has no file.
fn applies(root: &Path, spec: &TaskSpec) -> Option<Option<String>> {
    let found = if spec.files.is_empty() {
        None
    } else {
        Some(
            spec.files
                .iter()
                .find(|file| root.join(file).exists())?
                .clone(),
        )
    };
    let holds = spec.requires.iter().all(|file| root.join(file).exists())
        && !spec.absent.iter().any(|file| root.join(file).exists())
        && spec
            .text
            .iter()
            .all(|[file, pattern]| matches(root, file, pattern))
        && !spec
            .no_text
            .iter()
            .any(|[file, pattern]| matches(root, file, pattern));
    holds.then_some(found)
}

/// The keys of the JSON object at the path `path`, keys joined by dots, in the document `text`.
fn keys_of_json(text: &str, path: &str) -> Vec<String> {
    let Ok(mut at) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    for key in path.split('.') {
        let Some(next) = at.get(key).cloned() else {
            return Vec::new();
        };
        at = next;
    }
    at.as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

/// The keys of the tables of a TOML document at each of `paths`, keys joined by dots.
fn keys_of_toml(text: &str, paths: &[String]) -> Vec<String> {
    let table: toml::Table = text.parse().unwrap_or_default();
    paths
        .iter()
        .filter_map(|path| {
            let mut at: &toml::Value = table.get(path.split('.').next()?)?;
            for key in path.split('.').skip(1) {
                at = at.get(key)?;
            }
            at.as_table()
        })
        .flat_map(|table| table.keys().cloned())
        .collect()
}

/// The names `each` reads from the file `file` of `root`, each once.
fn names_of(root: &Path, file: &str, each: &Each) -> Vec<String> {
    let file = each.file.as_deref().unwrap_or(file);
    let Ok(text) = std::fs::read_to_string(root.join(file)) else {
        return Vec::new();
    };
    let mut names = if let Some(path) = &each.json {
        keys_of_json(&text, path)
    } else if !each.toml.is_empty() {
        keys_of_toml(&text, &each.toml)
    } else if let Some(pattern) = &each.regex {
        Regex::new(pattern)
            .map(|pattern| {
                pattern
                    .captures_iter(&text)
                    .filter_map(|found| found.name("name").map(|held| held.as_str().to_owned()))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut seen = Vec::new();
    names.retain(|name| {
        let fresh = !seen.contains(name);
        seen.push(name.clone());
        fresh
    });
    names.retain(|name| is_plain(name));
    names
}

/// The group the generated task called `name` belongs to, by the names `spec` gives each group.
fn group_of(spec: &TaskSpec, name: &str) -> String {
    spec.groups
        .iter()
        .find(|(_, names)| names.iter().any(|held| held == name))
        .map_or_else(|| spec.group.clone(), |(group, _)| group.clone())
}

/// The tasks the languages' `specs`, each with the family of toolchains it runs with, say the files in `root` imply, in the order the specs are given, a task whose name an earlier one has left out.
pub fn detect(root: &Path, specs: &[(String, TaskSpec)]) -> Vec<Task> {
    let mut found: Vec<Task> = Vec::new();
    for (family, spec) in specs {
        let Some(file) = applies(root, spec) else {
            continue;
        };
        let source = spec
            .source
            .clone()
            .or_else(|| file.clone())
            .or_else(|| spec.requires.first().cloned())
            .unwrap_or_default();
        let made: Vec<Task> = match (&spec.each, &file) {
            (Some(each), file) => {
                let file = file.clone().unwrap_or_default();
                names_of(root, &file, each)
                    .into_iter()
                    .map(|name| {
                        Task::new(
                            spec.name.replace("{name}", &name),
                            spec.command.replace("{name}", &name),
                            &group_of(spec, &name),
                            family,
                            &source,
                        )
                    })
                    .collect()
            }
            (None, _) => vec![Task::new(
                spec.name.clone(),
                spec.command.clone(),
                &spec.group,
                family,
                &source,
            )],
        };
        for task in made {
            if !found.iter().any(|held| held.is_named(&task.name)) {
                found.push(task);
            }
        }
    }
    found
}

/// Every task of `root`: those written in its own file, then those the languages' `specs` imply, leaving out a detected task a written one has the name of.
pub fn tasks(root: &Path, specs: &[(String, TaskSpec)]) -> Found {
    let (mut tasks, problem) = file::read(root);
    let written = tasks.len();
    for detected in detect(root, specs) {
        if !tasks[..written]
            .iter()
            .any(|own| own.is_named(&detected.name))
        {
            tasks.push(detected);
        }
    }
    Found { tasks, problem }
}
