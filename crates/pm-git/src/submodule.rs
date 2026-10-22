//! The submodules of a repository: finding them and checking them out in a worktree.

use std::path::Path;

use crate::run;

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

/// Puts the submodule at `path` in `worktree` on a new `branch` from the commit it has checked out, so that what is committed there is on a branch rather than a detached head.
pub fn branch_submodule(worktree: &Path, path: &str, branch: &str) -> Result<(), String> {
    run(&worktree.join(path), &["checkout", "-b", branch]).map(|_| ())
}
