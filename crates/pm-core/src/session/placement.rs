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

/// Atomically reserves an unused session directory, stepping past existing names.
pub fn reserve(worktrees: &Path, project: &str, name: &str) -> std::io::Result<PathBuf> {
    let directory = worktrees.join(slug(project));
    pm_host::Host::local().fs().create_dir_all(&directory)?;
    let wanted = slug(name);
    for nth in 1.. {
        let path = directory.join(if nth == 1 {
            wanted.clone()
        } else {
            format!("{wanted}-{nth}")
        });
        match pm_host::Host::local().fs().create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    unreachable!()
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
