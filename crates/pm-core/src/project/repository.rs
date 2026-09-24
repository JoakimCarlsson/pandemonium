//! What makes a path a repository: its root, the branch it has out, and the
//! repositories a folder holds.
//!
//! Only as much git as opening a project needs, read straight off the files
//! rather than shelled out to, so opening one is a handful of reads and never
//! waits on a subprocess. Anything richer belongs to the session and worktree
//! code, not here.
//!
//! A folder is searched a couple of levels down, the way VS Code looks for
//! repositories: a folder of services, each its own repository, is one
//! project holding several of them, not several projects.

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

/// How many directories below a project's root a repository is looked for.
const DEPTH: usize = 2;

/// Directories never searched for a repository: the trees every ecosystem
/// regenerates, which are large and never hold one worth reviewing.
const UNSEARCHED: &[&str] = &["node_modules", "target", "vendor", "venv", "dist", "build"];

/// The working-copy root at or above `path`, if one of them is a repository.
pub fn root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| is_root(ancestor))
        .map(Path::to_path_buf)
}

/// Whether `root` is itself the root of a repository's working copy.
fn is_root(root: &Path) -> bool {
    root.join(GIT).exists()
}

/// Every repository at or below `root`, the root's own first.
///
/// The rest come in path order. A repository inside another one is listed as
/// well, since git treats it as a repository of its own; a hidden directory,
/// a symlink and a dependency tree are never searched.
pub fn repositories(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    search(root, DEPTH, &mut found);
    found
}

/// Adds the repositories at or below `directory`, `depth` levels down, to
/// `found`.
fn search(directory: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if is_root(directory) {
        found.push(directory.to_path_buf());
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut children = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .filter(|path| searched(path))
        .collect::<Vec<_>>();
    children.sort();
    for child in children {
        search(&child, depth - 1, found);
    }
}

/// Whether the directory at `path` is worth looking inside for a repository.
fn searched(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| !name.starts_with('.') && !UNSEARCHED.contains(&name))
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
