//! The linked worktrees of a repository: adding one, tearing one down, and
//! how far one has drifted from what it was cut from.
//!
//! A session's worktree is git's own, made and removed by git rather than by
//! copying files about, so everything here is one `git worktree` run in the
//! repository the worktree belongs to. What a worktree remembers of itself —
//! what it was cut from, what it is called — is kept in a config file inside
//! its own git directory, which git carries from one launch to the next and
//! takes away with the worktree. The repository's config is not the place:
//! every worktree of a repository shares it, and a second session would
//! overwrite what the first had written down.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::git::Said;
use crate::git::run::{answer, git};

/// The config key a worktree remembers the commit it was cut from under.
const BASE_KEY: &str = "pandemonium.base";

/// The config key a worktree remembers what it is called under.
const NAME_KEY: &str = "pandemonium.name";

/// The config key a worktree remembers the port it was given under.
const PORT_KEY: &str = "pandemonium.port";

/// The line of `git worktree list --porcelain` naming a worktree's path.
const WORKTREE_LINE: &str = "worktree ";

/// The file inside a worktree's own git directory its memory is kept in.
const MEMORY: &str = "pandemonium.config";

/// How much of a commit hash names it where one is written down.
const SHORT_HASH: usize = 7;

/// How far a worktree has drifted from the commit it was cut from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Summary {
    /// How many files differ.
    pub files: usize,
    /// How many lines have been added across them.
    pub added: usize,
    /// How many lines have been taken out across them.
    pub removed: usize,
}

impl Summary {
    /// Whether the worktree still holds exactly what it was cut from.
    pub fn is_empty(self) -> bool {
        self.files == 0 && self.added == 0 && self.removed == 0
    }

    /// The drift as one line, the way a session row states it.
    ///
    /// A worktree that has changed nothing states it in the same shape as one
    /// that has changed a great deal, because the column is read by glancing
    /// down it: a blank is one more thing to work out.
    pub fn line(self) -> String {
        format!("+{} −{}", self.added, self.removed)
    }
}

/// Cuts a worktree at `path` from `base`, in the repository at `root`.
///
/// The head is left detached on purpose: a session starts as a place to work,
/// and the branch it ends up on is named when there is something to name it
/// after rather than before the agent has written a line.
pub fn add_worktree(root: &Path, path: &Path, base: &str) -> Said {
    git(
        root,
        [
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--detach"),
            path.as_os_str(),
            OsStr::new(base),
        ],
    )
}

/// Takes the worktree at `path` out of the repository at `root`.
///
/// Whatever the worktree holds goes with it, which is what finishing a
/// session means: the record of what the agent did is the branch or the
/// commits it pushed, never the directory it worked in.
///
/// A worktree already taken away outside the editor, its directory deleted
/// or git no longer listing it, counts as removed: git is only told to
/// forget the stale entry it may still hold.
pub fn remove_worktree(root: &Path, path: &Path) -> Said {
    if !is_linked(root, path) {
        let _ = answer(root, ["worktree", "prune"]);
        return Ok(String::new());
    }
    let removed = git(
        root,
        [
            OsStr::new("worktree"),
            OsStr::new("remove"),
            OsStr::new("--force"),
            path.as_os_str(),
        ],
    );
    let _ = answer(root, ["worktree", "prune"]);
    removed.map(|_| String::new())
}

/// Every linked worktree of the repository at `root`, in git's own order.
///
/// The repository's own checkout is left out: it is the project, not a
/// session, and it is the one worktree the window already knows about.
pub fn worktrees(root: &Path) -> Vec<PathBuf> {
    let Some(listed) = answer(root, ["worktree", "list", "--porcelain"]) else {
        return Vec::new();
    };
    listed
        .lines()
        .filter_map(|line| line.strip_prefix(WORKTREE_LINE))
        .map(PathBuf::from)
        .filter(|path| path != root)
        .collect()
}

/// Whether `path` is on disk and still one of the linked worktrees of the
/// repository at `root`.
///
/// Both sides are compared as the filesystem resolves them, since git lists
/// a worktree by its real path and a session may hold it by a linked one.
fn is_linked(root: &Path, path: &Path) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    worktrees(root)
        .iter()
        .any(|listed| listed.canonicalize().is_ok_and(|listed| listed == path))
}

/// The commit `revision` names in the repository at `root`, shortened.
pub fn commit_of(root: &Path, revision: &str) -> Option<String> {
    let commit = answer(root, ["rev-parse", revision])?;
    let commit = commit.trim();
    match commit.is_empty() {
        true => None,
        false => Some(commit.chars().take(SHORT_HASH).collect()),
    }
}

/// How far the worktree at `root` has drifted from `base`.
///
/// Committed and uncommitted work count alike, because a reader glancing at a
/// session wants to know how much of it there is, not how much of it the
/// agent has got round to committing.
pub fn since(root: &Path, base: &str) -> Summary {
    let Some(stat) = answer(root, ["diff", "--shortstat", base]) else {
        return Summary::default();
    };
    stat.split(',').fold(Summary::default(), |mut drift, part| {
        let Some(count) = part.split_whitespace().next().and_then(|c| c.parse().ok()) else {
            return drift;
        };
        match () {
            () if part.contains("file") => drift.files = count,
            () if part.contains("insertion") => drift.added = count,
            () if part.contains("deletion") => drift.removed = count,
            () => {}
        }
        drift
    })
}

/// Writes down what the worktree at `root` was cut from and is called.
pub fn remember(root: &Path, base: &str, name: &str) {
    write(root, BASE_KEY, base);
    write(root, NAME_KEY, name);
}

/// Writes down the port the worktree at `root` was given.
///
/// The port is the session's for as long as the worktree is, so it is kept
/// where the rest of what a worktree knows about itself is kept: a launch
/// that finds the worktree again hands its server the same port.
pub fn remember_port(root: &Path, port: u16) {
    write(root, PORT_KEY, &port.to_string());
}

/// The port the worktree at `root` was given, as it wrote it down.
pub fn remembered_port(root: &Path) -> Option<u16> {
    read(root, PORT_KEY)?.parse().ok()
}

/// The commit the worktree at `root` was cut from, as it wrote it down.
pub fn remembered_base(root: &Path) -> Option<String> {
    read(root, BASE_KEY)
}

/// What the worktree at `root` is called, as it wrote it down.
pub fn remembered_name(root: &Path) -> Option<String> {
    read(root, NAME_KEY)
}

/// The file the worktree at `root` keeps its memory in, inside the git
/// directory that is its alone.
fn memory(root: &Path) -> Option<PathBuf> {
    let directory = answer(root, ["rev-parse", "--absolute-git-dir"])?;
    let directory = directory.trim();
    match directory.is_empty() {
        true => None,
        false => Some(Path::new(directory).join(MEMORY)),
    }
}

/// Writes `value` under `key` in the memory of the worktree at `root`.
fn write(root: &Path, key: &str, value: &str) {
    if let Some(file) = memory(root) {
        let _ = answer(
            root,
            [
                OsStr::new("config"),
                OsStr::new("--file"),
                file.as_os_str(),
                OsStr::new(key),
                OsStr::new(value),
            ],
        );
    }
}

/// The value `key` holds in the memory of the worktree at `root`, if any.
fn read(root: &Path, key: &str) -> Option<String> {
    let file = memory(root)?;
    let value = answer(
        root,
        [
            OsStr::new("config"),
            OsStr::new("--file"),
            file.as_os_str(),
            OsStr::new("--get"),
            OsStr::new(key),
        ],
    )?;
    let value = value.trim();
    match value.is_empty() {
        true => None,
        false => Some(value.to_owned()),
    }
}
