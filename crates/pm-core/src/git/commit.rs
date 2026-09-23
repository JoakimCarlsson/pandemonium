//! Committing what the index holds, and what the last commit was called.
//!
//! Only what is staged is committed. A screen that wants everything in is
//! one that stages everything first, because the two are separate things to
//! have meant and a commit that quietly took more than it was shown is the
//! one mistake here that cannot be undone by hand.

use std::ffi::OsStr;
use std::path::Path;

use crate::git::graph::{Lanes, lanes};
use crate::git::run::{Said, answer, git};

/// One commit in the recent history of a worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commit {
    /// How the commit's row of the graph is drawn.
    pub lanes: Lanes,
    /// The abbreviated object name.
    pub id: String,
    /// The branches and tags pointing at the commit, as git decorates them.
    pub refs: Vec<String>,
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
///
/// Commits come in date order, which still lists every child before its
/// parents — the one thing laying out the lanes needs of the order.
pub fn history(root: &Path, limit: usize, all: bool) -> Vec<Commit> {
    let count = format!("-{}", limit.max(1));
    let mut arguments: Vec<&OsStr> = vec![
        OsStr::new("log"),
        OsStr::new(&count),
        OsStr::new("--date-order"),
        OsStr::new("--pretty=format:%H%x09%P%x09%h%x09%D%x09%s"),
    ];
    if all {
        arguments.push(OsStr::new("--all"));
    }
    let listed: Vec<Listed> = answer(root, arguments)
        .map(|said| said.lines().filter_map(listed).collect())
        .unwrap_or_default();
    let rows = lanes(
        listed
            .iter()
            .map(|commit| (commit.object.as_str(), commit.parents.as_slice())),
    );
    listed
        .into_iter()
        .zip(rows)
        .map(|(commit, lanes)| Commit {
            lanes,
            id: commit.id,
            refs: commit.refs,
            summary: commit.summary,
        })
        .collect()
}

/// One line of the history as git lists it, before the lanes are laid out.
struct Listed {
    /// The full object name.
    object: String,
    /// The full object names of the parents, first parent first.
    parents: Vec<String>,
    /// The abbreviated object name.
    id: String,
    /// The branches and tags pointing at the commit.
    refs: Vec<String>,
    /// The first line of the commit message.
    summary: String,
}

/// Reads one tab-separated line of the history.
fn listed(line: &str) -> Option<Listed> {
    let mut fields = line.splitn(5, '\t');
    let object = fields.next()?.to_owned();
    let parents = fields
        .next()?
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let id = fields.next()?.to_owned();
    let refs = fields
        .next()?
        .split(", ")
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    let summary = fields.next().unwrap_or_default().to_owned();
    Some(Listed {
        object,
        parents,
        id,
        refs,
        summary,
    })
}
