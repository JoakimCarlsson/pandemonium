//! What git makes of a worktree: where its head stands, and what has changed.
//!
//! One question answers both the tree, which wants a colour per name, and the
//! screens that review changes, which want the files themselves with each
//! side of the index kept apart. Asking git twice for one worktree would be
//! two subprocesses to say one thing, so the answer is read once into
//! [`Status`] and looked at from either side.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::git::head::Head;
use crate::git::run::answer;

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
    /// The same file, under another name.
    Renamed,
    /// Changed on both sides of a merge.
    Conflicted,
}

impl FileStatus {
    /// What one side of a porcelain status pair comes to.
    ///
    /// A dot is git saying that this side of the index holds nothing about
    /// the file, which is not a status but the absence of one.
    fn of(state: char) -> Option<Self> {
        match state {
            'M' | 'T' => Some(Self::Modified),
            'A' => Some(Self::Added),
            'D' => Some(Self::Deleted),
            'R' | 'C' => Some(Self::Renamed),
            _ => None,
        }
    }

    /// The letter this status is written with where one letter has to do.
    pub const fn letter(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Untracked => "U",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Conflicted => "C",
        }
    }

    /// How loudly this status asks to be noticed, against the others.
    ///
    /// A name wears one colour however many things are true of it, so the
    /// two sides of the index are settled by taking the louder of them.
    const fn urgency(self) -> u8 {
        match self {
            Self::Conflicted => 5,
            Self::Untracked => 4,
            Self::Deleted => 3,
            Self::Added => 2,
            Self::Renamed => 1,
            Self::Modified => 0,
        }
    }
}

/// One file git has something to say about, on either side of the index.
///
/// Both sides are kept because they are two different things to do: what the
/// index holds is what a commit would take, and what the worktree holds is
/// what is not in it yet. The same file is often both at once — staged, then
/// typed into again — and a list that showed only one of them would be
/// hiding half of what is about to be committed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Changed {
    /// The file itself, in the worktree it was found in.
    pub path: PathBuf,
    /// Where it came from, when it got here by being renamed.
    pub from: Option<PathBuf>,
    /// What the index holds about it, when the index holds anything.
    pub index: Option<FileStatus>,
    /// What the worktree holds about it that the index does not.
    pub worktree: Option<FileStatus>,
}

impl Changed {
    /// Whether some of this file is staged for the next commit.
    pub fn is_staged(&self) -> bool {
        self.index.is_some()
    }

    /// Whether some of this file is changed and not staged.
    pub fn is_unstaged(&self) -> bool {
        self.worktree.is_some()
    }

    /// Whether git has never been told about the file at all.
    pub fn is_untracked(&self) -> bool {
        self.worktree == Some(FileStatus::Untracked)
    }

    /// Whether the last commit has no such file at all.
    ///
    /// A file that was made and then staged is as new as one that was never
    /// staged: putting either of them back the way the last commit had it
    /// means taking it off the disk, because the last commit had nothing.
    pub fn is_created(&self) -> bool {
        self.is_untracked() || self.index == Some(FileStatus::Added)
    }

    /// Whether the file is changed on both sides of a merge.
    pub fn is_conflicted(&self) -> bool {
        self.index == Some(FileStatus::Conflicted)
    }

    /// The one status to write this file down as.
    pub fn mark(&self) -> FileStatus {
        match (self.index, self.worktree) {
            (Some(index), Some(worktree)) if worktree.urgency() > index.urgency() => worktree,
            (Some(index), _) => index,
            (None, Some(worktree)) => worktree,
            (None, None) => FileStatus::Modified,
        }
    }

    /// What the file is called where a row has room for one name.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Everything git has to say about one worktree right now.
pub struct Status {
    /// Where the worktree's head stands.
    head: Head,
    /// Every file git has something to say about, in path order.
    changed: Vec<Changed>,
    /// The one status to draw each path with, directories included.
    marks: HashMap<PathBuf, FileStatus>,
    /// The files and directories git ignores, with directory contents implied.
    ignored: HashSet<PathBuf>,
}

impl Default for Status {
    /// What a worktree git will not answer about comes to: nothing at all.
    fn default() -> Self {
        Self {
            head: Head::default(),
            changed: Vec::new(),
            marks: HashMap::new(),
            ignored: HashSet::new(),
        }
    }
}

impl Status {
    /// What git makes of the worktree at `root`.
    ///
    /// Version two of the porcelain format is asked for because it is the
    /// one that says where the head stands as well as what has changed, and
    /// keeps the two sides of the index apart rather than flattening them
    /// into one letter each.
    pub fn of(root: &Path) -> Self {
        let Some(text) = answer(
            root,
            [
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=all",
                "--ignored=matching",
            ],
        ) else {
            return Self::default();
        };

        let mut status = Self::default();
        status.head.operation = crate::git::operation(root);
        let mut fields = text.split('\0');
        while let Some(entry) = fields.next() {
            match entry.chars().next() {
                Some('#') => status.head.read(entry),
                Some('1') => status.take(read_ordinary(root, entry)),
                Some('2') => {
                    let from = fields.next().map(|from| root.join(from));
                    status.take(read_renamed(root, entry, from));
                }
                Some('u') => status.take(read_conflicted(root, entry)),
                Some('?') => status.take(read_untracked(root, entry)),
                Some('!') => {
                    if let Some(path) = entry.get(2..) {
                        status.ignored.insert(root.join(path));
                    }
                }
                _ => {}
            }
        }

        status.changed.sort_by(|one, two| one.path.cmp(&two.path));
        status.mark_directories();
        status
    }

    /// Where the worktree's head stands.
    pub fn head(&self) -> &Head {
        &self.head
    }

    /// Every file git has something to say about, in path order.
    pub fn changed(&self) -> &[Changed] {
        &self.changed
    }

    /// What git makes of `path`, if it makes anything of it.
    pub fn mark(&self, path: &Path) -> Option<FileStatus> {
        self.marks.get(path).copied()
    }

    /// Whether git ignores `path` itself or a directory containing it.
    pub fn is_ignored(&self, path: &Path) -> bool {
        path.ancestors().any(|path| self.ignored.contains(path))
    }

    /// Takes in one file, if the line it was read from was one.
    fn take(&mut self, changed: Option<Changed>) {
        if let Some(changed) = changed {
            self.marks.insert(changed.path.clone(), changed.mark());
            self.changed.push(changed);
        }
    }

    /// Marks every directory holding something that has changed.
    ///
    /// The tree can then say where to look without every directory being
    /// expanded first — as changed, whatever happened inside it, because a
    /// directory holding a deleted file has not itself been deleted.
    fn mark_directories(&mut self) {
        let inside = self
            .changed
            .iter()
            .map(|changed| (changed.path.clone(), changed.is_conflicted()))
            .collect::<Vec<_>>();
        for (path, conflicted) in inside {
            let held = match conflicted {
                true => FileStatus::Conflicted,
                false => FileStatus::Modified,
            };
            for parent in path.ancestors().skip(1) {
                let mark = self.marks.entry(parent.to_path_buf()).or_insert(held);
                if conflicted {
                    *mark = held;
                }
            }
        }
    }
}

/// One file changed in the ordinary way, read off a `1` entry.
///
/// The fields before the path are fixed in number and none of them holds a
/// space, so the path is whatever is left after them — which is what keeps a
/// file called `my notes.txt` one file rather than two.
fn read_ordinary(root: &Path, entry: &str) -> Option<Changed> {
    let mut fields = entry.splitn(9, ' ');
    let states = fields.nth(1)?;
    let path = fields.nth(6)?;
    Some(Changed {
        path: root.join(path),
        from: None,
        index: FileStatus::of(states.chars().next()?),
        worktree: FileStatus::of(states.chars().nth(1)?),
    })
}

/// One file that got where it is by being renamed, read off a `2` entry.
fn read_renamed(root: &Path, entry: &str, from: Option<PathBuf>) -> Option<Changed> {
    let mut fields = entry.splitn(10, ' ');
    let states = fields.nth(1)?;
    let path = fields.nth(7)?;
    Some(Changed {
        path: root.join(path),
        from,
        index: FileStatus::of(states.chars().next()?),
        worktree: FileStatus::of(states.chars().nth(1)?),
    })
}

/// One file changed on both sides of a merge, read off a `u` entry.
fn read_conflicted(root: &Path, entry: &str) -> Option<Changed> {
    let path = entry.splitn(11, ' ').nth(10)?;
    Some(Changed {
        path: root.join(path),
        from: None,
        index: Some(FileStatus::Conflicted),
        worktree: Some(FileStatus::Conflicted),
    })
}

/// One file git has never been told about, read off a `?` entry.
fn read_untracked(root: &Path, entry: &str) -> Option<Changed> {
    let path = entry.get(2..)?;
    Some(Changed {
        path: root.join(path),
        from: None,
        index: None,
        worktree: Some(FileStatus::Untracked),
    })
}
