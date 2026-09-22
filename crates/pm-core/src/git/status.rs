//! What git makes of each file of a worktree, for the tree that lists them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What git makes of one file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileStatus {
    /// Changed since the last commit.
    Modified,
    /// New, and already staged.
    Added,
    /// New, and git has never been told about it.
    Untracked,
    /// Gone from the worktree.
    Deleted,
    /// Changed on both sides of a merge.
    Conflicted,
}

impl FileStatus {
    /// What a porcelain status pair comes to.
    ///
    /// The two characters are what the index and the worktree each say; the
    /// tree shows one colour per name, so the more urgent of the two wins.
    fn of(staged: char, working: char) -> Option<Self> {
        match (staged, working) {
            ('U', _) | (_, 'U') | ('D', 'D') | ('A', 'A') => Some(Self::Conflicted),
            ('?', _) => Some(Self::Untracked),
            (_, 'D') | ('D', _) => Some(Self::Deleted),
            ('A', _) => Some(Self::Added),
            (' ', ' ') => None,
            _ => Some(Self::Modified),
        }
    }
}

/// What git makes of each file of the worktree at `root`.
///
/// A directory holding something changed is listed as changed itself, so the
/// tree can say where to look without every directory being expanded first —
/// as changed, whatever happened inside it, because a directory holding a
/// deleted file has not itself been deleted.
///
/// A rename is two fields rather than one: the path it went to, and then the
/// path it came from with no status in front of it. The second is stepped
/// over, or it would be read as a file whose status is its own name.
pub fn status(root: &Path) -> HashMap<PathBuf, FileStatus> {
    let Ok(output) = Command::new("git")
        .args(["status", "--porcelain", "-z", "--untracked-files=all"])
        .current_dir(root)
        .stderr(Stdio::null())
        .output()
    else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut found = HashMap::new();
    let mut fields = text.split('\0').peekable();

    while let Some(entry) = fields.next() {
        if entry.len() <= 3 {
            continue;
        }
        let mut characters = entry.chars();
        let (Some(staged), Some(working)) = (characters.next(), characters.next()) else {
            continue;
        };
        if matches!(staged, 'R' | 'C') {
            fields.next();
        }
        let Some(status) = FileStatus::of(staged, working) else {
            continue;
        };
        let path = root.join(entry.chars().skip(3).collect::<String>().trim());
        let held = match status {
            FileStatus::Conflicted => FileStatus::Conflicted,
            _ => FileStatus::Modified,
        };
        for parent in path.ancestors().skip(1).take_while(|above| *above != root) {
            let entry = found.entry(parent.to_path_buf()).or_insert(held);
            if held == FileStatus::Conflicted {
                *entry = held;
            }
        }
        found.insert(path, status);
    }
    found
}
