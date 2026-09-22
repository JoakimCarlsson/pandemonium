//! Every file of a worktree, for the surfaces that search all of them.
//!
//! The tree reads a directory when somebody opens it; a file palette and a
//! project-wide search need the whole worktree at once. Both skip the same
//! things, so what counts as worth listing is decided here: the repository's
//! own `.gitignore`, plus the handful of directories every checkout has that
//! nobody means to open.

use std::path::{Path, PathBuf};

/// The directories skipped whatever a repository says about them.
const ALWAYS_SKIPPED: &[&str] = &[".git", ".hg", ".svn"];

/// Most files a worktree is listed as holding before the walk gives up.
///
/// A walk is what a keystroke in the file palette waits on, so it is bounded
/// rather than complete: a checkout with a million files in it is one the
/// palette lists the first hundred thousand of.
const LIMIT: usize = 100_000;

/// Every file under `root`, in the order the directories were read.
pub fn walk(root: &Path) -> Vec<PathBuf> {
    let ignore = Ignore::read(root);
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if found.len() >= LIMIT {
                return found;
            }
            let path = entry.path();
            let directory = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if ignore.skips(&path, directory) {
                continue;
            }
            if directory {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found
}

/// What a worktree says is not worth listing.
///
/// This is not the whole of the gitignore format: a pattern anchored to a
/// subdirectory, or negated by a later one, is a rule for git to apply when
/// it decides what to commit. What a file palette needs is the common case —
/// a name, a directory or a suffix — and to be wrong quietly when the file
/// really is ignored but is listed anyway.
struct Ignore {
    /// The worktree the patterns were read from.
    root: PathBuf,
    /// The patterns themselves, as written.
    patterns: Vec<String>,
}

impl Ignore {
    /// The rules of the repository at `root`.
    fn read(root: &Path) -> Self {
        let patterns = std::fs::read_to_string(root.join(".gitignore"))
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
            .map(|line| line.trim_start_matches('/').to_owned())
            .filter(|line| line != "/")
            .filter(|line| !line.is_empty())
            .collect();

        Self {
            root: root.to_path_buf(),
            patterns,
        }
    }

    /// Whether `path` is one of the things not worth listing.
    fn skips(&self, path: &Path, directory: bool) -> bool {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return true;
        };
        if ALWAYS_SKIPPED.contains(&name) {
            return true;
        }
        let relative = path
            .strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();

        self.patterns
            .iter()
            .any(|pattern| matches(pattern, name, &relative, directory))
    }
}

/// Whether `pattern` covers a file called `name` at `relative`.
///
/// A rule written with a trailing slash is about directories alone, which is
/// the difference between hiding a `build` directory and hiding a file
/// somebody called `build`.
fn matches(pattern: &str, name: &str, relative: &str, directory: bool) -> bool {
    let (pattern, directories_only) = match pattern.strip_suffix('/') {
        Some(pattern) => (pattern, true),
        None => (pattern, false),
    };
    if directories_only && !directory {
        return false;
    }
    if pattern.contains('/') {
        return relative == pattern || relative.starts_with(&format!("{pattern}/"));
    }
    match pattern.strip_prefix('*') {
        Some(suffix) if !suffix.is_empty() => name.ends_with(suffix),
        Some(_) => false,
        None => name == pattern,
    }
}
