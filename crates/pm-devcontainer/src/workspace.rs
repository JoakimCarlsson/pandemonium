//! How a worktree's files and git metadata are mounted into its container.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// How deep below the source root a nested checkout's `.git` file is looked for.
const DEPTH: usize = 3;

/// Folders that never hold a checkout worth mounting the metadata of.
const SKIPPED: [&str; 5] = [".git", "node_modules", "target", ".venv", "build"];

/// One host folder made visible in the container.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Bind {
    /// The desktop folder.
    pub source: PathBuf,
    /// Where it appears in the container.
    pub target: String,
}

/// The extra binds a source folder needs for git to keep working in the container.
///
/// A linked worktree's `.git` file names a metadata folder inside the main
/// repository by absolute path, and that folder names the worktree back. Both
/// are mounted at the paths the files record, so nothing on the desktop is
/// rewritten: the pointer resolves, git's checks pass, and a prune from inside
/// the container sees the worktree where it expects it.
pub fn git_binds(source: &Path, workspace: &str) -> Vec<Bind> {
    let mut common = BTreeSet::new();
    let mut linked = false;
    for file in dot_git_files(source, 0) {
        let Some(target) = pointed_at(&file) else {
            continue;
        };
        linked = true;
        common.insert(common_directory(&target));
    }
    let mut binds = common
        .into_iter()
        .map(|directory| Bind {
            target: directory.display().to_string(),
            source: directory,
        })
        .collect::<Vec<_>>();
    if linked && source.display().to_string() != workspace {
        binds.push(Bind {
            source: source.to_path_buf(),
            target: source.display().to_string(),
        });
    }
    binds
}

/// The variables that keep git inside a container from pruning the metadata of worktrees it cannot see.
///
/// A linked worktree's siblings are not mounted, so git sees them as missing and
/// would prune their metadata once its grace period passes. Setting the period
/// to never through the environment leaves the repository's own configuration
/// untouched.
pub fn linked_environment(source: &Path) -> Vec<(String, String)> {
    if dot_git_files(source, 0).is_empty() {
        return Vec::new();
    }
    vec![
        ("GIT_CONFIG_COUNT".to_owned(), "1".to_owned()),
        (
            "GIT_CONFIG_KEY_0".to_owned(),
            "gc.worktreePruneExpire".to_owned(),
        ),
        ("GIT_CONFIG_VALUE_0".to_owned(), "never".to_owned()),
    ]
}

/// Every `.git` file, which marks a linked worktree or a submodule, at or below `directory`.
fn dot_git_files(directory: &Path, depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let marker = directory.join(".git");
    if marker.is_file() {
        found.push(marker);
    }
    if depth >= DEPTH {
        return found;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SKIPPED.contains(&name.as_ref()) || !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        found.extend(dot_git_files(&entry.path(), depth + 1));
    }
    found
}

/// The absolute folder a `.git` file points at.
fn pointed_at(file: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(file).ok()?;
    let target = text
        .lines()
        .find_map(|line| line.strip_prefix("gitdir:"))?
        .trim();
    let target = Path::new(target);
    Some(if target.is_absolute() {
        target.to_path_buf()
    } else {
        file.parent()?.join(target)
    })
}

/// The folder every worktree of a repository shares, from one worktree's metadata folder.
fn common_directory(gitdir: &Path) -> PathBuf {
    if let Ok(text) = std::fs::read_to_string(gitdir.join("commondir")) {
        let common = Path::new(text.trim());
        let joined = if common.is_absolute() {
            common.to_path_buf()
        } else {
            gitdir.join(common)
        };
        if let Ok(resolved) = joined.canonicalize() {
            return resolved;
        }
    }
    gitdir.to_path_buf()
}
