//! The files of a worktree kept under a reference of their own, and put back from one.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{run, run_indexed};

/// Who the commits holding kept files are by, so that no identity of the reader's is needed.
const IDENTITY: [&str; 4] = [
    "-c",
    "user.name=Pandemonium",
    "-c",
    "user.email=pandemonium@localhost",
];

/// The folder at the top of the worktree `directory` is in.
fn toplevel(directory: &Path) -> Result<PathBuf, String> {
    let top = run(directory, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(top.trim()))
}

/// Where the repository keeps its file `name` for the worktree `directory` is at the top of.
fn git_path(directory: &Path, name: &str) -> Result<PathBuf, String> {
    let path = run(directory, &["rev-parse", "--git-path", name])?;
    Ok(directory.join(path.trim()))
}

/// The paths of `listing`, which separates them with nul characters.
fn listed(listing: &str) -> Vec<String> {
    listing
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The tree of every file under `top` that the repository does not ignore, staged into an index of its own so that what the reader staged is left alone.
fn snapshot(top: &Path) -> Result<String, String> {
    let index = git_path(top, "pandemonium.index")?;
    let own = git_path(top, "index")?;
    if fs::copy(&own, &index).is_err() {
        let _ = fs::remove_file(&index);
    }
    let staged = run_indexed(top, &["add", "--all", "--", "."], Some(&index));
    let tree = staged.and_then(|_| run_indexed(top, &["write-tree"], Some(&index)));
    let _ = fs::remove_file(&index);
    Ok(tree?.trim().to_owned())
}

/// Keeps the files of the worktree `directory` is in as they are now, tracked or not, under the reference `name`.
///
/// Neither the branch, the head nor what is staged changes.
///
/// shortcut: the files inside submodules are not kept, only the commit each is at; keep them when a rewind has to undo edits made inside one.
pub fn checkpoint(directory: &Path, name: &str) -> Result<(), String> {
    let top = toplevel(directory)?;
    let tree = snapshot(&top)?;
    let mut arguments = IDENTITY.to_vec();
    arguments.extend(["commit-tree", &tree, "-m", "Files kept by Pandemonium"]);
    let commit = run(&top, &arguments)?;
    run(&top, &["update-ref", name, commit.trim()]).map(|_| ())
}

/// Puts the files of the worktree `directory` is in back as the reference `name` keeps them, after keeping them as they are under `backup`, and returns the files that changed, from the root of the repository.
///
/// Files the reference does not have are deleted; neither the branch, the head nor what is staged changes.
pub fn restore(directory: &Path, name: &str, backup: &str) -> Result<Vec<String>, String> {
    let top = toplevel(directory)?;
    let target = run(
        &top,
        &["rev-parse", "--verify", &format!("{name}^{{commit}}")],
    )?;
    let target = target.trim();
    checkpoint(&top, backup)?;
    let changed = listed(&run(
        &top,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            backup,
            target,
            "--",
        ],
    )?);
    let kept: HashSet<String> =
        listed(&run(&top, &["ls-tree", "-r", "--name-only", "-z", target])?)
            .into_iter()
            .collect();
    let (back, gone): (Vec<&String>, Vec<&String>) =
        changed.iter().partition(|path| kept.contains(*path));
    for path in gone {
        let file = top.join(path);
        if !file.is_dir() {
            fs::remove_file(&file).or_else(|error| match error.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(format!("could not delete {path}: {error}")),
            })?;
        }
    }
    if !back.is_empty() {
        let source = format!("--source={target}");
        let paths: Vec<String> = back
            .iter()
            .map(|path| format!(":(literal){path}"))
            .collect();
        let mut arguments = vec!["restore", &source, "--worktree", "--"];
        arguments.extend(paths.iter().map(String::as_str));
        run(&top, &arguments)?;
    }
    Ok(changed)
}
