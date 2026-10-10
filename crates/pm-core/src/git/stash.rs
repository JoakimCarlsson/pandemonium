//! Stashes saved in one worktree and the commands that restore them.

use pm_host::Location;

use crate::git::run::{Said, answer, git};

/// One saved stash entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stash {
    /// The stash's index in the current list.
    pub index: usize,
    /// The message shown for the stash.
    pub message: String,
    /// Its creation time as Unix seconds.
    pub when: i64,
}

/// Lists the stashes of the repository at `root` when requested.
pub fn stashes(root: impl Into<Location>) -> Vec<Stash> {
    let root = root.into();
    answer(&root, ["stash", "list", "--format=%gd%x09%ct%x09%gs"])
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let index = fields
                .next()?
                .strip_prefix("stash@{")?
                .strip_suffix('}')?
                .parse()
                .ok()?;
            let when = fields.next()?.parse().ok()?;
            let message = fields.next()?.to_owned();
            Some(Stash {
                index,
                message,
                when,
            })
        })
        .collect()
}

/// Saves changes from `root`, optionally including untracked files.
pub fn stash_push(root: impl Into<Location>, message: &str, untracked: bool) -> Said {
    let root = root.into();
    let mut arguments = vec!["stash", "push"];
    if untracked {
        arguments.push("--include-untracked");
    }
    if !message.trim().is_empty() {
        arguments.extend(["-m", message]);
    }
    git(&root, arguments)
}

/// Applies the stash at `index` without removing it.
pub fn stash_apply(root: impl Into<Location>, index: usize) -> Said {
    let root = root.into();
    git(&root, ["stash", "apply", &format!("stash@{{{index}}}")])
}

/// Applies and removes the stash at `index`.
pub fn stash_pop(root: impl Into<Location>, index: usize) -> Said {
    let root = root.into();
    git(&root, ["stash", "pop", &format!("stash@{{{index}}}")])
}

/// Removes the stash at `index`.
pub fn stash_drop(root: impl Into<Location>, index: usize) -> Said {
    let root = root.into();
    git(&root, ["stash", "drop", &format!("stash@{{{index}}}")])
}
