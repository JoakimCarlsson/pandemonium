//! Committing what the index holds, and what the last commit was called.
//!
//! Only what is staged is committed. A screen that wants everything in is
//! one that stages everything first, because the two are separate things to
//! have meant and a commit that quietly took more than it was shown is the
//! one mistake here that cannot be undone by hand.

use std::ffi::OsStr;
use std::path::Path;

use crate::git::run::{Said, answer, git};

/// One commit in the recent history of a worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commit {
    /// The graph lane marks Git draws before the commit.
    pub graph: String,
    /// The abbreviated object name.
    pub id: String,
    /// The first line of the commit message.
    pub summary: String,
}

/// Commits in the worktree at `root`, saying `message`.
///
/// With nothing staged there is nothing for a commit to take, so `tracked`
/// says to take every tracked file's changes instead — which is what the
/// button offering to commit them means, and the one case where a commit
/// takes something the reader did not put in the index.
///
/// An empty message is not sent to git at all: git would open an editor, and
/// there is no editor to open in a window that is one already.
pub fn commit(root: &Path, message: &str, tracked: bool) -> Said {
    if message.trim().is_empty() {
        return Err("A commit needs a message".to_owned());
    }
    let mut arguments = vec![OsStr::new("commit")];
    if tracked {
        arguments.push(OsStr::new("--all"));
    }
    arguments.push(OsStr::new("-m"));
    arguments.push(OsStr::new(message));

    git(root, arguments)
}

/// What the last commit of the worktree at `root` was called.
///
/// A worktree with nothing committed yet has nothing to say, which is what
/// a screen offering to write the last message again reads as having none.
pub fn last_message(root: &Path) -> Option<String> {
    let said = answer(root, ["log", "-1", "--pretty=%B"])?;
    let trimmed = said.trim().to_owned();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// The most recent `limit` commits in the worktree at `root`.
pub fn history(root: &Path, limit: usize, all: bool) -> Vec<Commit> {
    let count = format!("-{}", limit.max(1));
    let mut arguments: Vec<&OsStr> = vec![
        OsStr::new("log"),
        OsStr::new(&count),
        OsStr::new("--graph"),
        OsStr::new("--pretty=format:%h%x09%s"),
    ];
    if all {
        arguments.push(OsStr::new("--all"));
    }
    answer(root, arguments)
        .map(|commits| {
            commits
                .lines()
                .filter_map(|line| {
                    let (lane, summary) = line.split_once('\t')?;
                    let (graph, id) = lane.rsplit_once(' ')?;
                    Some(Commit {
                        graph: graph.to_owned(),
                        id: id.to_owned(),
                        summary: summary.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
