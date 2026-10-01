//! Every file of a worktree, for the surfaces that search all of them.
//!
//! The tree reads a directory when somebody opens it; a file palette and a
//! project-wide search need the whole worktree at once. Both skip the same
//! things, so what counts as worth listing is decided here: the repository's
//! own `.gitignore`, plus the handful of directories every checkout has that
//! nobody means to open.

use std::path::{Path, PathBuf};

use crate::ignore::Ignore;

/// Most files a worktree is listed as holding before the walk gives up.
///
/// A walk is what the file palette and a search list from, and what they
/// hold in memory, so it is bounded rather than complete: a checkout with a million files in it is one the
/// palette lists the first hundred thousand of.
const LIMIT: usize = 100_000;

/// Hands every file under `root` to `found` as it is come upon, in the
/// order the directories were read, until `found` answers that it wants no
/// more or the walk reaches its limit.
///
/// This is the walk for a caller that shows files while the rest are still
/// being listed, or that may be told to stop before the listing is done.
pub fn walk_each(root: &Path, mut found: impl FnMut(PathBuf) -> bool) {
    let ignore = Ignore::read(root);
    let mut listed = 0;
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if listed >= LIMIT {
                return;
            }
            let path = entry.path();
            let directory = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if ignore.skips(&path, directory) {
                continue;
            }
            if directory {
                pending.push(path);
                continue;
            }
            listed += 1;
            if !found(path) {
                return;
            }
        }
    }
}
