//! Worktree trees written through an index reserved for checkpoints.

use std::path::Path;

use super::run::{answer, git_with_index};

/// Writes the current tracked and untracked files, excluding ignored files.
/// Callers serialize snapshots for each worktree; neither HEAD nor its index is used.
pub fn snapshot(root: &Path) -> Option<String> {
    let root = root.canonicalize().ok()?;
    let named = answer(&root, ["rev-parse", "--git-path", "pandemonium.index"])?;
    let index = root.join(named.trim());
    if index.exists() {
        std::fs::remove_file(&index).ok()?;
    }
    git_with_index(&root, &index, ["add", "-A", "--", "."]).ok()?;
    let tree = git_with_index(&root, &index, ["write-tree"]).ok()?;
    Some(tree.trim().to_owned())
}
