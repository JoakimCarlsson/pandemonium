//! The index: what it holds for a file, and what is put into it.
//!
//! The index is the one thing between the worktree and a commit, so it is
//! also the one place staging, unstaging and throwing a change away belong.
//! Each of them is what git itself does — nothing here writes a file that
//! git would rather write — except for a file git has never been told about,
//! which has nothing in the index to be restored from and is thrown away by
//! being taken off the disk.

use std::ffi::OsStr;
use std::path::Path;

use crate::git::run::{Said, answer, git, holding, piped, within};

/// The text the index holds for `path`, in the repository holding it at or
/// below `root`.
///
/// A file that git has never heard of has no baseline, which is what makes
/// every line of a new file read as added rather than as unchanged.
pub fn baseline(root: &Path, path: &Path) -> Option<String> {
    let root = holding(root, path);
    let relative = within(&root, path)?;
    answer(&root, [OsStr::new("show"), &staged(relative)])
}

/// The text the last commit holds for `path`, in the repository holding it
/// at or below `root`.
///
/// A file the last commit does not have — one added since, or a repository
/// with no commit yet — has none.
pub fn committed(root: &Path, path: &Path) -> Option<String> {
    let root = holding(root, path);
    let relative = within(&root, path)?;
    let mut named = std::ffi::OsString::from("HEAD:");
    named.push(relative.as_os_str());
    answer(&root, [OsStr::new("show"), &named])
}

/// Puts what the worktree holds for `paths` into the index.
///
/// Adding is also how a file that has been deleted or renamed is staged:
/// git reads the worktree and writes down what it finds, including that
/// there is nothing there any more.
pub fn stage(root: &Path, paths: &[impl AsRef<Path>]) -> Said {
    run(root, &["add", "--"], paths)
}

/// Takes what the index holds for `paths` back out of it.
pub fn unstage(root: &Path, paths: &[impl AsRef<Path>]) -> Said {
    run(root, &["restore", "--staged", "--"], paths)
}

/// Puts `path` back the way the index holds it, losing what was typed into it.
///
/// A file the last commit never had has nothing to be put back to, so
/// throwing its changes away is throwing the file away: that is what the
/// worktree looked like before it was made.
pub fn discard(root: &Path, path: &Path, created: bool) -> Said {
    if created {
        return std::fs::remove_file(path)
            .map(|()| String::new())
            .map_err(|error| error.to_string());
    }
    run(root, &["restore", "--worktree", "--"], &[path])
}

/// Puts `content` into the index as what `path` holds.
///
/// This is how part of a file is staged: the text the index is to hold is
/// worked out first — what it holds now, with one run of lines replaced by
/// what the worktree has in their place — and written in whole. Git is given
/// the blob and then told that the path is that blob, which is the same pair
/// of commands `git add` runs for the whole of a file.
pub fn write_index(root: &Path, path: &Path, content: &str) -> Said {
    let Some(relative) = within(root, path) else {
        return Ok(String::new());
    };
    let named = relative.to_string_lossy().into_owned();
    let sha = piped(
        root,
        ["hash-object", "-w", "--stdin", "--path", &named],
        content,
    )?;

    git(
        root,
        [
            OsStr::new("update-index"),
            OsStr::new("--add"),
            OsStr::new("--cacheinfo"),
            OsStr::new(mode(path)),
            OsStr::new(sha.trim()),
            relative.as_os_str(),
        ],
    )
}

/// The mode git records for `path`: executable, or an ordinary file.
#[cfg(unix)]
fn mode(path: &Path) -> &'static str {
    use std::os::unix::fs::PermissionsExt;

    let executable = std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false);
    if executable { "100755" } else { "100644" }
}

/// The mode git records for `path`: always an ordinary file, since the
/// filesystem keeps no executable bit to read.
#[cfg(not(unix))]
fn mode(_path: &Path) -> &'static str {
    "100644"
}

/// Puts every one of `paths` back the way the index holds it.
///
/// Every file goes in one call: putting four files back is one thing to have
/// asked for, and a run that stopped halfway through would leave a worktree
/// nobody asked for.
pub fn discard_all(root: &Path, paths: &[impl AsRef<Path>]) -> Said {
    run(root, &["restore", "--worktree", "--"], paths)
}

/// Runs a git command over `paths`, each named from the worktree down.
///
/// A path outside the worktree is not git's to act on and is left out; a
/// call left with no paths at all is one that would otherwise have meant
/// every path, which is never what was asked for.
fn run(root: &Path, command: &[&str], paths: &[impl AsRef<Path>]) -> Said {
    let mut arguments = command.iter().map(OsStr::new).collect::<Vec<_>>();
    let within = paths
        .iter()
        .filter_map(|path| self::within(root, path.as_ref()))
        .collect::<Vec<_>>();
    if within.is_empty() {
        return Ok(String::new());
    }
    arguments.extend(within.iter().map(|path| path.as_os_str()));
    git(root, arguments)
}

/// How the index is named where git takes a revision.
fn staged(relative: &Path) -> std::ffi::OsString {
    let mut named = std::ffi::OsString::from(":");
    named.push(relative.as_os_str());
    named
}
