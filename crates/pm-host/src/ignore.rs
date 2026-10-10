//! What a worktree says is not worth listing, or watching.
//!
//! The walk that feeds the file palette and the watcher that follows the
//! disk skip the same things, so the rules are read here once for both.

use std::path::{Path, PathBuf};

/// The directories skipped whatever a repository says about them.
const ALWAYS_SKIPPED: &[&str] = &[".git", ".hg", ".svn"];

/// What a worktree says is not worth listing.
///
/// This is not the whole of the gitignore format: a pattern anchored to a
/// subdirectory, or negated by a later one, is a rule for git to apply when
/// it decides what to commit. What a file palette needs is the common case —
/// a name, a directory or a suffix — and to be wrong quietly when the file
/// really is ignored but is listed anyway.
///
/// The worktree's own `.gitignore` is read, and so is the one at the root of
/// every repository inside it: a folder of several repositories keeps what
/// each of them ignores in each of them.
pub(super) struct Ignore {
    /// The worktree the patterns were read for.
    root: PathBuf,
    /// Each directory a `.gitignore` was read from, with its patterns.
    rules: Vec<(PathBuf, Vec<String>)>,
}

impl Ignore {
    /// The rules of the worktree at `root` and the repositories inside it.
    pub(super) fn read(root: &Path) -> Self {
        let mut directories = vec![root.to_path_buf()];
        directories.extend(
            crate::filesystem::repositories(root)
                .into_iter()
                .filter(|repository| repository != root),
        );
        let rules = directories
            .into_iter()
            .map(|directory| {
                let patterns = patterns(&directory);
                (directory, patterns)
            })
            .filter(|(_, patterns)| !patterns.is_empty())
            .collect();

        Self {
            root: root.to_path_buf(),
            rules,
        }
    }

    /// Whether `path` is one of the things not worth listing.
    pub(super) fn skips(&self, path: &Path, directory: bool) -> bool {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return true;
        };
        if ALWAYS_SKIPPED.contains(&name) {
            return true;
        }
        self.rules.iter().any(|(base, patterns)| {
            let Ok(relative) = path.strip_prefix(base) else {
                return false;
            };
            let relative = relative.to_string_lossy();
            patterns
                .iter()
                .any(|pattern| matches(pattern, name, &relative, directory))
        })
    }

    /// Whether `path`, or any directory between it and the root, is skipped.
    ///
    /// A walk never reaches inside a skipped directory, so asking about the
    /// entry alone is enough for it; a change on disk can come from anywhere
    /// below one, and has to be asked about every step of the way down.
    pub(super) fn covers(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return false;
        };
        let mut above = self.root.clone();
        let mut steps = relative.components().peekable();
        while let Some(step) = steps.next() {
            above.push(step);
            let directory = steps.peek().is_some() || above.is_dir();
            if self.skips(&above, directory) {
                return true;
            }
        }
        false
    }
}

/// The patterns the `.gitignore` in `directory` holds, as written.
fn patterns(directory: &Path) -> Vec<String> {
    std::fs::read_to_string(directory.join(".gitignore"))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
        .map(|line| line.trim_start_matches('/').to_owned())
        .filter(|line| !line.is_empty())
        .collect()
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
