//! Carrying the ignored files a project names, such as `.env` or `target/`, into a new worktree.

use std::fs;
use std::path::Path;

use crate::{WORKTREES, ignored};

/// Whether `text` is matched by `pattern`, where `*` stands for any run of characters other than `/`.
fn matches(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => text.strip_prefix(head).is_some_and(|rest| {
            let reach = rest.find('/').unwrap_or(rest.len());
            (0..=reach)
                .filter(|&at| rest.is_char_boundary(at))
                .any(|at| matches(tail, &rest[at..]))
        }),
    }
}

/// Whether the ignored entry `entry`, written from the repository's root, is one `patterns` name: each pattern is a path from the root, with a `/` at its end for a folder.
fn named(patterns: &[String], entry: &str) -> bool {
    let entry = entry.trim_end_matches('/');
    patterns
        .iter()
        .any(|pattern| matches(pattern.trim().trim_matches('/'), entry))
}

/// Copies `from` to `to`, a folder with everything in it, sharing blocks with the original where the filesystem allows; a symbolic link is not followed and not copied.
fn copy(from: &Path, to: &Path) -> Result<(), String> {
    let kind = fs::symlink_metadata(from).map_err(|error| error.to_string())?;
    if kind.is_dir() {
        fs::create_dir_all(to).map_err(|error| error.to_string())?;
        for child in fs::read_dir(from).map_err(|error| error.to_string())? {
            let child = child.map_err(|error| error.to_string())?;
            copy(&child.path(), &to.join(child.file_name()))?;
        }
        Ok(())
    } else if kind.is_file() {
        reflink_copy::reflink_or_copy(from, to)
            .map(drop)
            .map_err(|error| error.to_string())
    } else {
        Ok(())
    }
}

/// Whether no part of `entry`, a path from the worktree's root, is a symbolic link there, so that writing it cannot land outside the worktree.
fn free_of_links(worktree: &Path, entry: &str) -> bool {
    let mut at = worktree.to_path_buf();
    entry.split('/').all(|part| {
        at.push(part);
        !fs::symlink_metadata(&at).is_ok_and(|kind| kind.is_symlink())
    })
}

/// Copies each ignored file or folder of `repository` that `patterns` name into the same place in `worktree`, and returns each that could not be copied, as its path and why.
///
/// A pattern is a path from the repository's root, in which `*` stands for any run of characters in one name; only what git ignores is copied, never what is tracked, and never git's own folder or the folder worktrees are made in. A path that does not exist is left out without a word. A file inside a folder git ignores as a whole is named by naming the folder.
pub(crate) fn carry_ignored(
    repository: &Path,
    worktree: &Path,
    patterns: &[String],
) -> Vec<(String, String)> {
    if patterns.is_empty() {
        return Vec::new();
    }
    ignored(repository, usize::MAX)
        .into_iter()
        .filter(|entry| named(patterns, entry))
        .map(|entry| entry.trim_end_matches('/').to_owned())
        .filter(|entry| entry != ".git" && !WORKTREES.starts_with(entry.as_str()))
        .filter(|entry| fs::symlink_metadata(worktree.join(entry)).is_err())
        .filter(|entry| free_of_links(worktree, entry))
        .filter_map(|entry| {
            copy(&repository.join(&entry), &worktree.join(&entry))
                .err()
                .map(|reason| (entry, reason))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Placement, add_worktree};
    use std::process::Command;

    /// Runs `git` with `arguments` in `directory` and expects it to succeed.
    fn git(directory: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(arguments)
            .current_dir(directory)
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// Wildcards stand for part of one name and nothing else.
    #[test]
    fn wildcards_stay_inside_a_name() {
        assert!(matches(".env.*", ".env.local"));
        assert!(matches("*.log", "a.log"));
        assert!(!matches(".env.*", ".env"));
        assert!(!matches("*.log", "dir/a.log"));
        assert!(matches("target", "target"));
    }

    /// Listed ignored paths reach the new worktree; tracked, unlisted and missing ones do not break it.
    #[test]
    fn listed_ignored_paths_are_copied_into_a_new_worktree() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build")
            .join(format!("carry-{}", std::process::id()));
        fs::create_dir_all(root.join("target/debug")).unwrap();
        git(&root, &["init", "-q"]);
        fs::write(root.join(".gitignore"), ".env*\ntarget/\nother\n").unwrap();
        fs::write(root.join("tracked.rs"), "").unwrap();
        fs::write(root.join(".env"), "KEY=1\n").unwrap();
        fs::write(root.join(".env.local"), "KEY=2\n").unwrap();
        fs::write(root.join("other"), "").unwrap();
        fs::write(root.join("target/debug/out"), "built").unwrap();
        git(&root, &["add", ".gitignore", "tracked.rs"]);
        git(&root, &["commit", "-q", "-m", "first"]);
        let placement = Placement {
            include: vec![
                ".env".to_owned(),
                ".env.*".to_owned(),
                "target/".to_owned(),
                "tracked.rs".to_owned(),
                "missing".to_owned(),
            ],
            ..Placement::default()
        };
        let made = add_worktree(&root, "carry", "HEAD", &placement).unwrap();
        assert!(made.skipped.is_empty());
        assert_eq!(
            fs::read_to_string(made.path.join(".env")).unwrap(),
            "KEY=1\n"
        );
        assert_eq!(
            fs::read_to_string(made.path.join(".env.local")).unwrap(),
            "KEY=2\n"
        );
        assert_eq!(
            fs::read_to_string(made.path.join("target/debug/out")).unwrap(),
            "built"
        );
        assert!(!made.path.join("other").exists());
        let plain = add_worktree(&root, "plain", "HEAD", &Placement::default()).unwrap();
        assert!(!plain.path.join(".env").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
