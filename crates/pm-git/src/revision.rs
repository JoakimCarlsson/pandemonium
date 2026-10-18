//! The files a commit changed, or a branch changed since it left another.

use std::path::Path;

use crate::{Change, hunks, run};

/// `name` when it can name a commit or a branch and nothing else: not empty, not an option, and free of white space and control characters.
fn named(name: &str) -> Result<&str, String> {
    let plain = !name.is_empty()
        && !name.starts_with('-')
        && !name
            .chars()
            .any(|character| character.is_whitespace() || character.is_control());
    if plain {
        Ok(name)
    } else {
        Err(format!("{name:?} does not name a commit"))
    }
}

/// The files that differ between `from` and `to` in the repository `directory` is in, each with its diff.
///
/// A branch is counted from where it left `from`, and a commit with no `from` from its first parent.
pub fn revision_changes(
    directory: &Path,
    from: Option<&str>,
    to: &str,
) -> Result<Vec<Change>, String> {
    let to = named(to)?;
    let range = match from {
        Some(from) => format!("{}...{to}", named(from)?),
        None => format!("{to}^..{to}"),
    };
    let listed = match run(
        directory,
        &["diff", "--name-status", "-z", "-M", "--no-color", &range],
    ) {
        Ok(listed) => listed,
        Err(_) if from.is_none() => run(
            directory,
            &[
                "diff-tree",
                "--root",
                "-r",
                "--name-status",
                "-z",
                "-M",
                "--no-commit-id",
                to,
            ],
        )?,
        Err(reason) => return Err(reason),
    };
    let mut found = Vec::new();
    let mut entries = listed.split('\0').filter(|entry| !entry.is_empty());
    while let Some(status) = entries.next() {
        let letter = status.chars().next().unwrap_or('M');
        let renamed = matches!(letter, 'R' | 'C');
        let first = entries.next().unwrap_or_default();
        let (path, before) = if renamed {
            (entries.next().unwrap_or(first), Some(first.to_owned()))
        } else {
            (first, None)
        };
        let paths: Vec<&str> = before
            .as_deref()
            .into_iter()
            .chain(std::iter::once(path))
            .collect();
        let mut arguments = vec!["diff", "-M", "--no-color", &range, "--"];
        arguments.extend(&paths);
        let shown = run(directory, &arguments)
            .or_else(|_| {
                let mut root = vec!["show", "--format=", "-M", "--no-color", to, "--"];
                root.extend(&paths);
                run(directory, &root)
            })
            .unwrap_or_default();
        let diff = hunks(&shown);
        found.push(Change {
            path: path.to_owned(),
            words: crate::words(&diff),
            diff,
            staged: false,
            untracked: false,
            index: Some(letter),
            worktree: None,
            conflicted: false,
            from: before,
        });
    }
    Ok(found)
}
