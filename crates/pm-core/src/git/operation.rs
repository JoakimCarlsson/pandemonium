//! In-progress git operations in one worktree.

use pm_host::Location;

use std::path::{Path, PathBuf};

use crate::git::run::{Said, answer, git};

/// An operation waiting for its next step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    /// A merge awaiting conflict resolution or a commit.
    Merge(Merge),
    /// A rebase replaying commits.
    Rebase(Rebase),
    /// A cherry-pick awaiting conflict resolution.
    CherryPick(CherryPick),
}

/// The merge pending in one worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Merge {
    /// The incoming commit's object name.
    pub incoming: String,
    /// The message git prepared for the merge commit.
    pub message: String,
}

/// The rebase pending in one worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rebase {
    /// The target commit or branch.
    pub onto: String,
    /// The branch being rebased.
    pub branch: String,
    /// The current replay step.
    pub step: usize,
    /// The total replay steps.
    pub total: usize,
    /// The message of the commit currently being replayed.
    pub message: String,
}

/// The cherry-pick pending in one worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CherryPick {
    /// The commit being picked.
    pub commit: String,
    /// The message git prepared for the picked commit.
    pub message: String,
}

/// Resolves a git control file or directory in `root`, including linked worktrees.
fn git_path(root: impl Into<Location>, name: &str) -> Option<PathBuf> {
    let root = root.into();
    let path = answer(&root, ["rev-parse", "--git-path", name])?;
    let path = Path::new(path.trim());
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    })
}

/// Reads `name` in the worktree's git directory.
fn git_file(root: impl Into<Location>, name: &str) -> Option<String> {
    let root = root.into();
    root.host
        .fs()
        .read_to_string(git_path(&root, name)?)
        .ok()
        .map(|value| value.trim().to_owned())
}

/// Reads the message git prepared, without its instructional comment lines.
fn prepared_message(root: impl Into<Location>) -> String {
    let root = root.into();
    git_file(&root, "MERGE_MSG")
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// Reads the operation pending in `root`, if any.
pub fn operation(root: impl Into<Location>) -> Option<Operation> {
    let root = root.into();
    for directory in ["rebase-merge", "rebase-apply"] {
        if root.host.fs().is_dir(git_path(&root, directory)?) {
            let read = |name: &str| git_file(&root, &format!("{directory}/{name}"));
            let onto = read("onto").unwrap_or_default();
            let onto = answer(&root, ["name-rev", "--name-only", &onto])
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty() && name != "undefined")
                .unwrap_or(onto);
            return Some(Operation::Rebase(Rebase {
                onto,
                branch: read("head-name")
                    .unwrap_or_default()
                    .trim_start_matches("refs/heads/")
                    .to_owned(),
                step: read("msgnum")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0),
                total: read("end")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0),
                message: read("message")
                    .or_else(|| {
                        answer(&root, ["log", "-1", "--pretty=%B", "REBASE_HEAD"])
                            .map(|value| value.trim().to_owned())
                    })
                    .unwrap_or_default(),
            }));
        }
    }
    if let Some(commit) = git_file(&root, "CHERRY_PICK_HEAD") {
        return Some(Operation::CherryPick(CherryPick {
            commit,
            message: prepared_message(&root),
        }));
    }
    let incoming = git_file(&root, "MERGE_HEAD")?;
    Some(Operation::Merge(Merge {
        incoming,
        message: prepared_message(&root),
    }))
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
            Self::Rebase(rebase) => format!(
                "Rebasing {}/{} onto {}",
                rebase.step, rebase.total, rebase.onto
            ),
            Self::CherryPick(pick) => format!(
                "Cherry-picking {}",
                &pick.commit[..pick.commit.len().min(7)]
            ),
        }
    }

    /// The verb naming this operation in menu actions.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Merge(_) => "Merge",
            Self::Rebase(_) => "Rebase",
            Self::CherryPick(_) => "Cherry-Pick",
        }
    }
}

/// Writes a chosen message and advances the operation in `root`.
pub fn continue_operation(root: impl Into<Location>, operation: &Operation) -> Said {
    let root = root.into();
    match operation {
        Operation::Merge(merge) => git(&root, ["commit", "-m", &merge.message]),
        Operation::Rebase(_) => git(&root, ["-c", "core.editor=true", "rebase", "--continue"]),
        Operation::CherryPick(pick) => {
            let path = git_path(&root, "MERGE_MSG").ok_or("Cannot find MERGE_MSG")?;
            root.host
                .fs()
                .write(path, &pick.message)
                .map_err(|error| error.to_string())?;
            git(
                &root,
                ["-c", "core.editor=true", "cherry-pick", "--continue"],
            )
        }
    }
}

/// Skips the stopped commit in a rebase or cherry-pick.
pub fn skip_operation(root: impl Into<Location>, operation: &Operation) -> Said {
    let root = root.into();
    match operation {
        Operation::Rebase(_) => git(&root, ["rebase", "--skip"]),
        Operation::CherryPick(_) => git(&root, ["cherry-pick", "--skip"]),
        Operation::Merge(_) => Err("A merge cannot skip a commit".to_owned()),
    }
}

/// Aborts the operation pending in `root`.
pub fn abort_operation(root: impl Into<Location>, operation: &Operation) -> Said {
    let root = root.into();
    match operation {
        Operation::Merge(_) => git(&root, ["merge", "--abort"]),
        Operation::Rebase(_) => git(&root, ["rebase", "--abort"]),
        Operation::CherryPick(_) => git(&root, ["cherry-pick", "--abort"]),
    }
}

/// Aborts the merge pending in `root`.
pub fn abort_merge(root: impl Into<Location>) -> Said {
    let root = root.into();
    git(&root, ["merge", "--abort"])
}
