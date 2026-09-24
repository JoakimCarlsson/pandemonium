//! What makes a path a repository: its root, and the branch it has out.
//!
//! Only as much git as opening a project needs, read straight off the files
//! rather than shelled out to, so opening one is a handful of reads and never
//! waits on a subprocess. Anything richer belongs to the session and worktree
//! code, not here.

use std::path::{Path, PathBuf};

/// The directory or file every repository keeps at its root.
const GIT: &str = ".git";

/// The file inside the git directory naming what is checked out.
const HEAD: &str = "HEAD";

/// The prefix `HEAD` carries when it names a branch instead of a commit.
const REF_PREFIX: &str = "ref: refs/heads/";

/// The prefix a linked worktree's `.git` file carries.
const GITDIR_PREFIX: &str = "gitdir:";

/// What a detached `HEAD` is called, having no branch to name.
const DETACHED: &str = "detached";

/// What a branch is called when `HEAD` cannot be read at all.
const UNKNOWN: &str = "unknown";

/// How much of a commit hash names it in place of a branch.
const SHORT_HASH: usize = 7;

/// The working-copy root at or above `path`, if one of them is a repository.
pub fn root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| is_root(ancestor))
        .map(Path::to_path_buf)
}

/// Whether `root` is itself the root of a repository's working copy.
pub fn is_root(root: &Path) -> bool {
    root.join(GIT).exists()
}

/// The branch checked out in the repository at `root`.
pub fn branch(root: &Path) -> String {
    let Some(head) = git_directory(root).map(|directory| directory.join(HEAD)) else {
        return UNKNOWN.to_owned();
    };
    let Ok(content) = std::fs::read_to_string(head) else {
        return UNKNOWN.to_owned();
    };
    let content = content.trim();

    match content.strip_prefix(REF_PREFIX) {
        Some(branch) => branch.to_owned(),
        None if content.len() >= SHORT_HASH => format!("{DETACHED} {}", &content[..SHORT_HASH]),
        None => DETACHED.to_owned(),
    }
}

/// The git directory of the working copy at `root`, following a worktree link.
fn git_directory(root: &Path) -> Option<PathBuf> {
    let git = root.join(GIT);
    if git.is_dir() {
        return Some(git);
    }

    let content = std::fs::read_to_string(&git).ok()?;
    let linked = content.trim().strip_prefix(GITDIR_PREFIX)?.trim();
    Some(root.join(linked))
}
