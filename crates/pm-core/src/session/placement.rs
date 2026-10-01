//! Where a session's worktree goes, and what it is called on disk.
//!
//! A worktree never sits beside the repository it was cut from: the project
//! is the truth, and a directory that appears next to it is something the
//! reader has to learn to ignore in every tool they own. They go under the
//! editor's own home instead, one directory per project, so a session is a
//! path anybody can guess and nothing the project can trip over.

use std::path::{Path, PathBuf};

/// What a session is called on disk when its name is nothing but punctuation.
const UNNAMED: &str = "session";

/// The characters a name keeps; every other one becomes a dash.
fn kept(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
}

/// The worktree of a session called `name`, of `project`, under `worktrees`.
///
/// A path already taken is stepped past rather than reused: two sessions of
/// one project may well be called the same thing, and the second of them is
/// still a worktree of its own.
pub fn place(worktrees: &Path, project: &str, name: &str) -> PathBuf {
    let directory = worktrees.join(slug(project));
    let wanted = slug(name);
    let taken = directory.join(&wanted);
    if !pm_host::Host::local().fs().exists(&taken) {
        return taken;
    }

    (2..)
        .map(|nth| directory.join(format!("{wanted}-{nth}")))
        .find(|path| !pm_host::Host::local().fs().exists(path))
        .unwrap_or(taken)
}

/// `name` as one path segment: kept characters, and dashes for the rest.
pub fn slug(name: &str) -> String {
    let dashed = name
        .chars()
        .map(|character| match kept(character) {
            true => character.to_ascii_lowercase(),
            false => '-',
        })
        .collect::<String>();

    let collapsed = dashed.split('-').filter(|part| !part.is_empty());
    let slug = collapsed.collect::<Vec<_>>().join("-");
    let trimmed = slug.trim_matches('.');

    match trimmed.is_empty() {
        true => UNNAMED.to_owned(),
        false => trimmed.to_owned(),
    }
}
