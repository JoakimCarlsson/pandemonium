//! Every file of a worktree, for the surfaces that search all of them.
//!
//! The tree reads a directory when somebody opens it; a file palette and a
//! project-wide search need the whole worktree at once. Both skip the same
//! things, so what counts as worth listing is decided here: the repository's
//! own `.gitignore`, plus the handful of directories every checkout has that
//! nobody means to open.

use std::path::{Path, PathBuf};

use crate::files::ignore::Ignore;

/// Most files a worktree is listed as holding before the walk gives up.
///
/// A walk is what a keystroke in the file palette waits on, so it is bounded
/// rather than complete: a checkout with a million files in it is one the
/// palette lists the first hundred thousand of.
const LIMIT: usize = 100_000;

/// Every file under `root`, in the order the directories were read.
pub fn walk(root: &Path) -> Vec<PathBuf> {
    let ignore = Ignore::read(root);
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if found.len() >= LIMIT {
                return found;
            }
            let path = entry.path();
            let directory = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if ignore.skips(&path, directory) {
                continue;
            }
            if directory {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found
}
