//! The submodules of a repository: finding them, checking them out in a worktree, telling what changed in them and carrying out git in the one a file is in.

use std::path::{Path, PathBuf};

use crate::{Change, Moved, run};

/// Where every submodule of `repository` is, nested ones included, from the root of the repository, in the order git lists them; none when it has none or is no repository.
pub fn submodules(repository: &Path) -> Vec<String> {
    run(repository, &["submodule", "status", "--recursive"])
        .map(|listed| listed.lines().filter_map(listed_path).collect())
        .unwrap_or_default()
}

/// The path a line of `git submodule status` names: what follows the state letter and the commit, up to the description in brackets.
fn listed_path(line: &str) -> Option<String> {
    let (_, rest) = line.get(1..)?.split_once(' ')?;
    let path = rest.rsplit_once(" (").map_or(rest, |(path, _)| path);
    (!path.is_empty()).then(|| path.to_owned())
}

/// The submodules `worktree` itself declares, without the ones nested in them.
fn declared(worktree: &Path) -> Vec<String> {
    run(
        worktree,
        &[
            "config",
            "--file",
            ".gitmodules",
            "--get-regexp",
            r"^submodule\..*\.path$",
        ],
    )
    .map(|listed| {
        listed
            .lines()
            .filter_map(|line| line.split_once(' ').map(|(_, path)| path.to_owned()))
            .collect()
    })
    .unwrap_or_default()
}

/// Checks out every submodule of the new `worktree` at the commit it records, nested ones included, and returns each that could not be, as its path and why.
///
/// A submodule on a local path is fetched only when the reader's git configuration allows the `file` transport, since a cloned repository can name any repository on the machine.
pub fn populate(worktree: &Path) -> Vec<(String, String)> {
    declared(worktree)
        .into_iter()
        .filter_map(|path| {
            run(
                worktree,
                &["submodule", "update", "--init", "--recursive", "--", &path],
            )
            .err()
            .map(|reason| {
                let why = reason
                    .lines()
                    .find(|line| line.starts_with("fatal:"))
                    .unwrap_or(&reason)
                    .trim_start_matches("fatal: ")
                    .to_owned();
                (path, why)
            })
        })
        .collect()
}

/// Puts every checked out submodule of `worktree`, nested ones included, on a new `branch` from the commit it is at, so that what is committed in one is on a branch rather than a detached head, and returns each that could not be, as its path and why.
pub(crate) fn branch_all(worktree: &Path, branch: &str) -> Vec<(String, String)> {
    submodules(worktree)
        .into_iter()
        .filter(|path| populated(&worktree.join(path)))
        .filter_map(|path| {
            run(&worktree.join(&path), &["checkout", "-b", branch])
                .err()
                .map(|reason| (path, reason))
        })
        .collect()
}

/// What each checked out submodule of `repository`, nested ones included, has checked out: its path from the root and its branch, or `@` and the commit on a detached head.
pub fn submodule_heads(repository: &Path) -> Vec<(String, String)> {
    submodules(repository)
        .into_iter()
        .filter_map(|path| {
            let folder = repository.join(&path);
            if !populated(&folder) {
                return None;
            }
            let head = match crate::current_branch(&folder) {
                Some(branch) => branch,
                None => format!("@ {}", short(&folder, &commit_of(&folder, "HEAD")?)),
            };
            Some((path, head))
        })
        .collect()
}

/// Where each submodule `repository` itself declares is, without the ones nested in them, from its root, as git lists them.
pub(crate) fn direct(repository: &Path) -> Vec<String> {
    run(repository, &["submodule", "status"])
        .map(|listed| listed.lines().filter_map(listed_path).collect())
        .unwrap_or_default()
}

/// Whether the submodule folder `folder` is checked out: it holds a repository of its own rather than being an empty folder of the one around it.
pub(crate) fn populated(folder: &Path) -> bool {
    folder.join(".git").exists()
}

/// The repository the file at `path` of `directory` is in, of those whose submodules are at `paths`: the folder of the innermost submodule holding it and the path from there, or `directory` and `path` themselves. A submodule's own path is in the repository around it.
fn owner(directory: &Path, paths: &[String], path: &str) -> (PathBuf, String) {
    paths
        .iter()
        .filter(|held| {
            path.strip_prefix(held.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|held| held.len())
        .map_or_else(
            || (directory.to_path_buf(), path.to_owned()),
            |held| (directory.join(held), path[held.len() + 1..].to_owned()),
        )
}

/// The repository the file at `path` of `directory` is in, and the path from there.
pub(crate) fn owner_of(directory: &Path, path: &str) -> (PathBuf, String) {
    owner(directory, &submodules(directory), path)
}

/// `paths` of `directory` sorted by the repository each is in, the repository itself or one of its submodules: each repository's folder with the paths from there, in the order they first come.
pub(crate) fn by_owner(directory: &Path, paths: &[&str]) -> Vec<(PathBuf, Vec<String>)> {
    let held = submodules(directory);
    let mut groups: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for path in paths {
        let (folder, inner) = owner(directory, &held, path);
        match groups.iter_mut().find(|(known, _)| *known == folder) {
            Some((_, group)) => group.push(inner),
            None => groups.push((folder, vec![inner])),
        }
    }
    groups
}

/// The commit `reference` names in `directory`, in full.
fn commit_of(directory: &Path, reference: &str) -> Option<String> {
    run(directory, &["rev-parse", "--verify", "--quiet", reference])
        .ok()
        .map(|said| said.trim().to_owned())
        .filter(|said| !said.is_empty())
}

/// `commit` abbreviated as the repository at `folder` would, or cut short when it does not have it.
fn short(folder: &Path, commit: &str) -> String {
    run(folder, &["rev-parse", "--short", commit])
        .map(|said| said.trim().to_owned())
        .unwrap_or_else(|_| commit.chars().take(7).collect())
}

/// The subjects of the commits `range` names in the repository at `folder`, oldest first.
fn subjects(folder: &Path, range: &str) -> Vec<String> {
    run(folder, &["log", "--reverse", "--format=%s", range, "--"])
        .map(|log| log.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// The entry git lists for the submodule `change` is of in `directory`, told as the commits it moved through, or nothing when it is checked out at the commit recorded and only what is inside it changed.
pub(crate) fn moved(directory: &Path, mut change: Change) -> Option<Change> {
    let folder = directory.join(&change.path);
    if !populated(&folder) {
        return None;
    }
    let current = commit_of(&folder, "HEAD")?;
    let recorded = commit_of(directory, &format!("HEAD:{}", change.path));
    let indexed = commit_of(directory, &format!(":{}", change.path));
    let index = (indexed != recorded).then_some(if recorded.is_some() { 'M' } else { 'A' });
    let worktree = (indexed.as_deref() != Some(current.as_str())).then_some('M');
    if index.is_none() && worktree.is_none() {
        return None;
    }
    let forward = recorded.as_deref().map_or_else(Vec::new, |from| {
        subjects(&folder, &format!("{from}..{current}"))
    });
    let back = forward.is_empty() && recorded.is_some();
    let commits = match (&recorded, back) {
        (Some(from), true) => subjects(&folder, &format!("{current}..{from}")),
        _ => forward,
    };
    change.moved = Some(Moved {
        from: recorded.as_deref().map(|from| short(&folder, from)),
        to: short(&folder, &current),
        commits,
        back,
    });
    change.diff = String::new();
    change.words = Vec::new();
    change.staged = index.is_some() && worktree.is_none();
    change.index = index;
    change.worktree = worktree;
    Some(change)
}

/// `change`, of a file of the submodule at `path`, as one of the repository around it.
pub(crate) fn within(mut change: Change, path: &str) -> Change {
    change.path = format!("{path}/{}", change.path);
    change.from = change.from.map(|from| format!("{path}/{from}"));
    change.submodule = Some(match change.submodule {
        Some(inner) => format!("{path}/{inner}"),
        None => path.to_owned(),
    });
    change
}

/// Pushes each checked out submodule of `directory` that is on a branch with commits no remote has, the innermost first, so that the repository around it never records a commit only this machine has.
pub(crate) fn push_inside(directory: &Path) -> Result<(), String> {
    let mut paths = submodules(directory);
    paths.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for path in &paths {
        let folder = directory.join(path);
        let unpublished = run(
            &folder,
            &["rev-list", "--count", "HEAD", "--not", "--remotes"],
        )
        .is_ok_and(|count| count.trim() != "0");
        if populated(&folder) && crate::current_branch(&folder).is_some() && unpublished {
            crate::push(&folder).map_err(|reason| format!("{path}: {reason}"))?;
        }
    }
    Ok(())
}
