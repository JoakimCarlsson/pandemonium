//! A folder that is no repository itself but holds repositories side by side: telling it from a repository and finding the repository a path is in.

use std::fs;
use std::path::{Path, PathBuf};

use crate::{SKIPPED, is_repository};

/// What a folder is as far as git goes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Layout {
    /// A repository, or a folder inside one.
    Repository,
    /// Not a repository, with repositories as direct children: each one's folder name, in name order.
    Repositories(Vec<String>),
    /// Neither a repository nor a folder holding repositories.
    Neither,
}

/// Whether `directory` is a repository, a folder of repositories or neither.
///
/// A folder of repositories is found by a shallow scan of its direct children that have a `.git` entry, a folder or the file a worktree has, leaving out hidden and bulky folders; repositories nested deeper are not looked for.
pub fn layout(directory: &Path) -> Layout {
    if directory.join(".git").exists() || is_repository(directory) {
        return Layout::Repository;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return Layout::Neither;
    };
    let mut found: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.') && !SKIPPED.contains(&name.as_str()))
        .filter(|name| directory.join(name).join(".git").exists())
        .collect();
    if found.is_empty() {
        return Layout::Neither;
    }
    found.sort();
    Layout::Repositories(found)
}

/// The repositories `directory` holds side by side, each one's folder name, or nothing when it is a repository, or neither.
pub fn child_repositories(directory: &Path) -> Vec<String> {
    children_of(directory).unwrap_or_default()
}

/// The repositories `directory` holds when it is a folder of repositories, and nothing when it is not.
pub(crate) fn children_of(directory: &Path) -> Option<Vec<String>> {
    match layout(directory) {
        Layout::Repositories(children) => Some(children),
        Layout::Repository | Layout::Neither => None,
    }
}

/// The repository that holds `path`, and `path` from there: the child of `directory` named by its first part when `children` are those of a folder of repositories, or `directory` and `path` themselves when `children` is nothing.
pub(crate) fn split(
    directory: &Path,
    children: Option<&[String]>,
    path: &str,
) -> Result<(PathBuf, String), String> {
    let Some(children) = children else {
        return Ok((directory.to_path_buf(), path.to_owned()));
    };
    match path.split_once('/') {
        Some((child, rest)) if !rest.is_empty() && children.iter().any(|held| held == child) => {
            Ok((directory.join(child), rest.to_owned()))
        }
        _ => Err(format!("{path} is in none of the repositories here")),
    }
}

/// The repository of `directory` that holds `path`, and `path` from there.
pub(crate) fn holder(directory: &Path, path: &str) -> Result<(PathBuf, String), String> {
    split(directory, children_of(directory).as_deref(), path)
}

/// The paths `list` finds in each of `children`, written from `directory`, at most `limit` of them.
pub(crate) fn gathered(
    directory: &Path,
    children: &[String],
    limit: usize,
    list: impl Fn(&Path, usize) -> Vec<String>,
) -> Vec<String> {
    children
        .iter()
        .flat_map(|child| {
            list(&directory.join(child), limit)
                .into_iter()
                .map(move |path| format!("{child}/{path}"))
        })
        .take(limit)
        .collect()
}

/// Runs `act` in each of `children` of `directory`, all of them even when one fails, and says which failed and why, each by name.
pub(crate) fn each(
    directory: &Path,
    children: &[String],
    mut act: impl FnMut(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let failures: Vec<String> = children
        .iter()
        .filter_map(|child| {
            act(&directory.join(child))
                .err()
                .map(|reason| format!("{child}: {reason}"))
        })
        .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}
