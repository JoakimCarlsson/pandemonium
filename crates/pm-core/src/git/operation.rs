//! In-progress git operations in one worktree.

use std::path::Path;

use crate::git::run::{Said, answer, git};

/// An operation waiting for its final commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    /// A merge awaiting conflict resolution or a commit.
    Merge(Merge),
}

/// The merge pending in one worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Merge {
    /// The incoming commit's object name.
    pub incoming: String,
    /// The message git prepared for the merge commit.
    pub message: String,
}

/// Reads the operation pending in `root`, if any.
pub fn operation(root: &Path) -> Option<Operation> {
    let path = answer(root, ["rev-parse", "--git-path", "MERGE_HEAD"])?;
    let path = Path::new(path.trim());
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let incoming = std::fs::read_to_string(path).ok()?.trim().to_owned();
    let path = answer(root, ["rev-parse", "--git-path", "MERGE_MSG"])?;
    let path = Path::new(path.trim());
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let message = std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned();
    Some(Operation::Merge(Merge { incoming, message }))
}

impl Operation {
    /// A short label for the operation in progress.
    pub fn label(&self) -> String {
        match self {
            Self::Merge(merge) => {
                let summary = merge.message.lines().next().unwrap_or_default();
                summary.strip_prefix("Merge ").map_or_else(
                    || "Merging".to_owned(),
                    |incoming| format!("Merging {incoming}"),
                )
            }
        }
    }
}

/// Aborts the merge pending in `root`.
pub fn abort_merge(root: &Path) -> Said {
    git(root, ["merge", "--abort"])
}
