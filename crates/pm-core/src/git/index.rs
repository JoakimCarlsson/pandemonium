//! The index: what it holds for a file, and what is put into it.
//!
//! The index is the one thing between the worktree and a commit, so it is
//! also the one place staging, unstaging and throwing a change away belong.
//! Each of them is what git itself does — nothing here writes a file that
//! git would rather write — except for a file git has never been told about,
//! which has nothing in the index to be restored from and is thrown away by
//! being taken off the disk.

use pm_host::Location;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::git::run::{Said, answer, git, holding, piped, streamed, within};

/// Where [`contents`] reads a file's text from.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Revision {
    /// The last commit, as [`committed`] reads it.
    Head,
    /// The index, as [`baseline`] reads it.
    Index,
}

impl Revision {
    /// How git is asked for `relative` in this revision, when it can be
    /// asked on a line of its own.
    fn naming(self, relative: &Path) -> Option<String> {
        let relative = relative.to_str().filter(|named| !named.contains('\n'))?;
        Some(match self {
            Self::Head => format!("HEAD:{relative}"),
            Self::Index => format!(":{relative}"),
        })
    }
}

/// The text the index holds for `path`, in the repository holding it at or
/// below `root`.
///
/// A file that git has never heard of has no baseline, which is what makes
/// every line of a new file read as added rather than as unchanged.
pub fn baseline(root: impl Into<Location>, path: &Path) -> Option<String> {
    let root = root.into();
    let root = holding(&root, path);
    let relative = within(&root, path)?;
    answer(&root, [OsStr::new("show"), &staged(relative)])
}

/// The text the last commit holds for `path`, in the repository holding it
/// at or below `root`.
///
/// A file the last commit does not have — one added since, or a repository
/// with no commit yet — has none.
pub fn committed(root: impl Into<Location>, path: &Path) -> Option<String> {
    let root = root.into();
    let root = holding(&root, path);
    let relative = within(&root, path)?;
    let mut named = std::ffi::OsString::from("HEAD:");
    named.push(relative.as_os_str());
    answer(&root, [OsStr::new("show"), &named])
}

/// The text each of `wanted` holds for its file, in the repository holding
/// it at or below `root`, answered in the order they were asked for.
///
/// This is [`committed`] and [`baseline`] for many files at once: one
/// `git cat-file --batch` per repository rather than one `git show` per
/// file, because a review of two thousand changed files would otherwise be
/// four thousand subprocesses. A file the revision does not have answers
/// nothing, as it does there.
pub fn contents(root: impl Into<Location>, wanted: &[(&Path, Revision)]) -> Vec<Option<String>> {
    let root = root.into();
    let mut answers = vec![None; wanted.len()];
    let mut asked = BTreeMap::<PathBuf, Vec<(usize, String)>>::new();
    for (at, (path, revision)) in wanted.iter().enumerate() {
        let repository = holding(&root, path);
        if let Some(named) =
            within(&repository, path).and_then(|relative| revision.naming(relative))
        {
            asked.entry(repository.path).or_default().push((at, named));
        }
    }
    for (repository, asked) in asked {
        let input = asked
            .iter()
            .map(|(_, named)| format!("{named}\n"))
            .collect::<String>();
        let Some(output) = streamed(root.at(&repository), ["cat-file", "--batch"], input) else {
            continue;
        };
        for ((at, _), text) in asked.iter().zip(batched(&output)) {
            answers[*at] = text;
        }
    }
    answers
}

/// Each answer `git cat-file --batch` wrote, in the order it wrote them:
/// the text of a blob, or nothing for a name it could not find or that is
/// not a file.
///
/// Every answer starts with a line saying what was found. A blob's is
/// `<id> blob <size>`, followed by that many bytes and a newline; a name git
/// has nothing for is `<name> missing` and ends there.
fn batched(output: &[u8]) -> Vec<Option<String>> {
    let mut answers = Vec::new();
    let mut rest = output;
    while let Some(end) = rest.iter().position(|byte| *byte == b'\n') {
        let header = String::from_utf8_lossy(&rest[..end]).into_owned();
        rest = &rest[end + 1..];
        let mut words = header.rsplitn(3, ' ');
        let size = words.next().and_then(|size| size.parse::<usize>().ok());
        let kind = words.next();
        let Some(size) = size.filter(|_| words.next().is_some()) else {
            answers.push(None);
            continue;
        };
        let Some(body) = rest.get(..size) else {
            break;
        };
        answers.push((kind == Some("blob")).then(|| String::from_utf8_lossy(body).into_owned()));
        rest = rest.get(size + 1..).unwrap_or_default();
    }
    answers
}

/// Puts what the worktree holds for `paths` into the index.
///
/// Adding is also how a file that has been deleted or renamed is staged:
/// git reads the worktree and writes down what it finds, including that
/// there is nothing there any more.
pub fn stage(root: impl Into<Location>, paths: &[impl AsRef<Path>]) -> Said {
    let root = root.into();
    run(&root, &["add", "--"], paths)
}

/// Takes what the index holds for `paths` back out of it.
pub fn unstage(root: impl Into<Location>, paths: &[impl AsRef<Path>]) -> Said {
    let root = root.into();
    run(&root, &["restore", "--staged", "--"], paths)
}

/// Puts `path` back the way the index holds it, losing what was typed into it.
///
/// A file the last commit never had has nothing to be put back to, so
/// throwing its changes away is throwing the file away: that is what the
/// worktree looked like before it was made.
pub fn discard(root: impl Into<Location>, path: &Path, created: bool) -> Said {
    let root = root.into();
    if created {
        return root
            .host
            .fs()
            .remove_file(path)
            .map(|()| String::new())
            .map_err(|error| error.to_string());
    }
    run(&root, &["restore", "--worktree", "--"], &[path])
}

/// Puts `content` into the index as what `path` holds.
///
/// This is how part of a file is staged: the text the index is to hold is
/// worked out first — what it holds now, with one run of lines replaced by
/// what the worktree has in their place — and written in whole. Git is given
/// the blob and then told that the path is that blob, which is the same pair
/// of commands `git add` runs for the whole of a file.
pub fn write_index(root: impl Into<Location>, path: &Path, content: &str) -> Said {
    let root = root.into();
    let Some(relative) = within(&root, path) else {
        return Ok(String::new());
    };
    let named = relative.to_string_lossy().into_owned();
    let sha = piped(
        &root,
        ["hash-object", "-w", "--stdin", "--path", &named],
        content,
    )?;

    git(
        &root,
        [
            OsStr::new("update-index"),
            OsStr::new("--add"),
            OsStr::new("--cacheinfo"),
            OsStr::new(mode(&root, path)),
            OsStr::new(sha.trim()),
            relative.as_os_str(),
        ],
    )
}

/// The mode git records for a file on its execution machine.
fn mode(root: &Location, path: &Path) -> &'static str {
    if root
        .host
        .fs()
        .metadata(path)
        .is_ok_and(|metadata| metadata.executable)
    {
        "100755"
    } else {
        "100644"
    }
}

/// Puts every one of `paths` back the way the index holds it.
///
/// Every file goes in one call: putting four files back is one thing to have
/// asked for, and a run that stopped halfway through would leave a worktree
/// nobody asked for.
pub fn discard_all(root: impl Into<Location>, paths: &[impl AsRef<Path>]) -> Said {
    let root = root.into();
    run(&root, &["restore", "--worktree", "--"], paths)
}

/// Runs a git command over `paths`, each named from the worktree down.
///
/// A path outside the worktree is not git's to act on and is left out; a
/// call left with no paths at all is one that would otherwise have meant
/// every path, which is never what was asked for.
fn run(root: impl Into<Location>, command: &[&str], paths: &[impl AsRef<Path>]) -> Said {
    let root = root.into();
    let mut arguments = command.iter().map(OsStr::new).collect::<Vec<_>>();
    let within = paths
        .iter()
        .filter_map(|path| self::within(&root, path.as_ref()))
        .collect::<Vec<_>>();
    if within.is_empty() {
        return Ok(String::new());
    }
    arguments.extend(within.iter().map(|path| path.as_os_str()));
    git(&root, arguments)
}

/// How the index is named where git takes a revision.
fn staged(relative: &Path) -> std::ffi::OsString {
    let mut named = std::ffi::OsString::from(":");
    named.push(relative.as_os_str());
    named
}
