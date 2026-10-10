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

use pm_host::Location;

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

/// The file inside a worktree's own git directory its review comments are
/// kept in, beside its memory.
const REVIEW: &str = "pandemonium.review";

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

/// Work a reader would lose by removing a worktree.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkAtRisk {
    /// Files with uncommitted changes, including untracked files.
    pub uncommitted_files: usize,
    /// Commits reachable from HEAD but from no remote-tracking ref.
    pub unpushed_commits: usize,
}

impl Summary {
    /// Whether the worktree still holds exactly what it was cut from.
    pub fn is_empty(self) -> bool {
        self.files == 0 && self.added == 0 && self.removed == 0
    }
}

/// Cuts a worktree at `path` from `base`, in the repository at `root`.
///
/// The head is left detached on purpose: a session starts as a place to work,
/// and the branch it ends up on is named when there is something to name it
/// after rather than before the agent has written a line.
pub fn add_worktree(root: impl Into<Location>, path: &Path, base: &str) -> Said {
    let root = root.into();
    git(
        &root,
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
pub fn remove_worktree(root: impl Into<Location>, path: &Path) -> Said {
    let root = root.into();
    if !is_linked(&root, path) {
        let _ = answer(&root, ["worktree", "prune"]);
        return Ok(String::new());
    }
    let removed = git(
        &root,
        [
            OsStr::new("worktree"),
            OsStr::new("remove"),
            OsStr::new("--force"),
            path.as_os_str(),
        ],
    );
    let _ = answer(&root, ["worktree", "prune"]);
    removed.map(|_| String::new())
}

/// Every linked worktree of the repository at `root`, in git's own order.
///
/// The repository's own checkout is left out: it is the project, not a
/// session, and it is the one worktree the window already knows about.
pub fn worktrees(root: impl Into<Location>) -> Vec<PathBuf> {
    let root = root.into();
    let Some(listed) = answer(&root, ["worktree", "list", "--porcelain"]) else {
        return Vec::new();
    };
    listed
        .lines()
        .filter_map(|line| line.strip_prefix(WORKTREE_LINE))
        .map(PathBuf::from)
        .filter(|path| path != &root.path)
        .collect()
}

/// Whether `path` is on disk and still one of the linked worktrees of the
/// repository at `root`.
///
/// Both sides are compared as the filesystem resolves them, since git lists
/// a worktree by its real path and a session may hold it by a linked one.
fn is_linked(root: impl Into<Location>, path: &Path) -> bool {
    let root = root.into();
    let fs = root.host.fs();
    let Ok(path) = fs.canonicalize(path) else {
        return false;
    };
    worktrees(&root)
        .iter()
        .any(|listed| fs.canonicalize(listed).is_ok_and(|listed| listed == path))
}

/// The commit `revision` names in the repository at `root`, shortened.
pub fn commit_of(root: impl Into<Location>, revision: &str) -> Option<String> {
    let root = root.into();
    let commit = answer(&root, ["rev-parse", revision])?;
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
pub fn since(root: impl Into<Location>, base: &str) -> Summary {
    let root = root.into();
    let Some(stat) = answer(&root, ["diff", "--shortstat", base]) else {
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

/// Work in the worktree at `root` that is absent from every remote.
///
/// Remote-tracking refs reflect the most recent fetch or push; this question
/// does not contact a remote when the reader opens the finish prompt.
pub fn work_at_risk(root: impl Into<Location>) -> WorkAtRisk {
    let root = root.into();
    let uncommitted_files = answer(&root, ["status", "--porcelain", "--untracked-files=all"])
        .map_or(0, |status| status.lines().count());
    let unpushed_commits = answer(&root, ["rev-list", "--count", "HEAD", "--not", "--remotes"])
        .and_then(|count| count.trim().parse().ok())
        .unwrap_or(0);
    WorkAtRisk {
        uncommitted_files,
        unpushed_commits,
    }
}

/// Writes down what the worktree at `root` was cut from and is called.
pub fn remember(root: impl Into<Location>, base: &str, name: &str) {
    let root = root.into();
    write(&root, BASE_KEY, base);
    write(&root, NAME_KEY, name);
}

/// Writes down the port the worktree at `root` was given.
///
/// The port is the session's for as long as the worktree is, so it is kept
/// where the rest of what a worktree knows about itself is kept: a launch
/// that finds the worktree again hands its server the same port.
pub fn remember_port(root: impl Into<Location>, port: u16) {
    let root = root.into();
    write(&root, PORT_KEY, &port.to_string());
}

/// The port the worktree at `root` was given, as it wrote it down.
pub fn remembered_port(root: impl Into<Location>) -> Option<u16> {
    let root = root.into();
    read(&root, PORT_KEY)?.parse().ok()
}

/// The commit the worktree at `root` was cut from, as it wrote it down.
pub fn remembered_base(root: impl Into<Location>) -> Option<String> {
    let root = root.into();
    read(&root, BASE_KEY)
}

/// What the worktree at `root` is called, as it wrote it down.
pub fn remembered_name(root: impl Into<Location>) -> Option<String> {
    let root = root.into();
    read(&root, NAME_KEY)
}

/// Writes down the review comments of the worktree at `root`, as text the
/// caller understands and this does not.
///
/// The file lives beside the worktree's memory, so it goes away with the
/// worktree. Nothing at all to say removes it rather than leaving it empty.
pub fn remember_review(root: impl Into<Location>, text: &str) {
    let root = root.into();
    let Some(file) = memory(&root).map(|memory| memory.with_file_name(REVIEW)) else {
        return;
    };
    match text.is_empty() {
        true => {
            let _ = root.host.fs().remove_file(file);
        }
        false => {
            let _ = root.host.fs().write(file, text);
        }
    }
}

/// The review comments the worktree at `root` wrote down, as it wrote them.
pub fn remembered_review(root: impl Into<Location>) -> Option<String> {
    let root = root.into();
    let file = memory(&root)?.with_file_name(REVIEW);
    root.host
        .fs()
        .read_to_string(file)
        .ok()
        .filter(|text| !text.is_empty())
}

/// The file the worktree at `root` keeps its memory in, inside the git
/// directory that is its alone.
fn memory(root: impl Into<Location>) -> Option<PathBuf> {
    let root = root.into();
    let directory = answer(&root, ["rev-parse", "--absolute-git-dir"])?;
    let directory = directory.trim();
    match directory.is_empty() {
        true => None,
        false => Some(Path::new(directory).join(MEMORY)),
    }
}

/// Writes `value` under `key` in the memory of the worktree at `root`.
fn write(root: impl Into<Location>, key: &str, value: &str) {
    let root = root.into();
    if let Some(file) = memory(&root) {
        let _ = answer(
            &root,
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
fn read(root: impl Into<Location>, key: &str) -> Option<String> {
    let root = root.into();
    let file = memory(&root)?;
    let value = answer(
        &root,
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

/// Writes durable session ancestry beside worktree memory, reporting storage failures.
pub(crate) fn remember_delegation(root: &Path, text: &str) -> Result<(), String> {
    let file = memory(root)
        .ok_or("The session has no git memory directory")?
        .with_file_name("pandemonium.delegation.json");
    let pending = file.with_extension("pending");
    let result = std::fs::write(&pending, format!("{text}\n"))
        .and_then(|()| {
            std::fs::File::options()
                .write(true)
                .open(&pending)?
                .sync_all()
        })
        .and_then(|()| std::fs::rename(&pending, &file));
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result.map_err(|error| error.to_string())
}

/// Reads durable session ancestry beside worktree memory.
pub(crate) fn remembered_delegation(root: &Path) -> Option<String> {
    std::fs::read_to_string(memory(root)?.with_file_name("pandemonium.delegation.json")).ok()
}
