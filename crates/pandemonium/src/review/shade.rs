//! What colour every character of a diff is drawn in.
//!
//! A diff shows lines out of three texts — the last commit, the index and
//! the worktree — and each line is coloured as it reads in the text it came
//! from, parsed whole, because a line of a hunk on its own is a fragment no
//! grammar can read. The worktree's lines are coloured again from the file's
//! open document once a language server has said what its names are, so a
//! change reads the way the same file does in an editor pane.

use std::collections::BTreeMap;
use std::path::Path;

use pm_core::{Hunk, Line, LineKind};
use pm_text::{Buffer, Highlight};

use crate::review::store::Patch;

/// Which of the three texts a line of a diff was read out of.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Version {
    /// What the last commit holds.
    Committed,
    /// What the index holds.
    Indexed,
    /// What is on disk.
    Worktree,
}

impl Version {
    /// The version the line `line` of a hunk on the `staged` side is read
    /// from, and which line of it it is, counted from one.
    ///
    /// A staged hunk takes the commit to the index and an unstaged one the
    /// index to the worktree; a removed line is on the older of the two and
    /// every other line on the newer.
    pub fn of(staged: bool, line: &Line) -> Option<(Self, usize)> {
        let (old, new) = match staged {
            true => (Self::Committed, Self::Indexed),
            false => (Self::Indexed, Self::Worktree),
        };
        match line.kind {
            LineKind::Removed => Some((old, line.old?)),
            LineKind::Added | LineKind::Context => Some((new, line.new?)),
        }
    }

    /// The text this version holds for `path`, in the worktree at `root`.
    fn read(self, root: &Path, path: &Path) -> Option<String> {
        match self {
            Self::Committed => pm_core::committed(root, path),
            Self::Indexed => pm_core::baseline(root, path),
            Self::Worktree => std::fs::read_to_string(path).ok(),
        }
    }
}

/// The colour of every character of one file's diff, line by line.
#[derive(Default)]
pub struct Shading {
    /// The highlight of each character of each line shown, by where the
    /// line was read from.
    lines: BTreeMap<(Version, usize), Vec<Option<Highlight>>>,
}

impl Shading {
    /// Reads `path` in every version `patch` shows a line of, and colours
    /// those lines.
    pub fn of(root: &Path, path: &Path, patch: &Patch) -> Self {
        let mut shading = Self::default();
        for (version, shown) in shown(patch) {
            let Some(text) = version.read(root, path) else {
                continue;
            };
            let mut buffer = Buffer::holding(path, &text);
            shading.paint(version, &shown, &mut buffer);
        }
        shading
    }

    /// Colours the worktree's lines of `patch` again from `buffer`, what a
    /// language server has said about its names included.
    pub fn repaint_worktree(&mut self, patch: &Patch, buffer: &mut Buffer) {
        if let Some(shown) = shown(patch).remove(&Version::Worktree) {
            self.paint(Version::Worktree, &shown, buffer);
        }
    }

    /// The highlights of the `line`-th line of `version`, counted from one.
    pub fn line(&self, version: Version, line: usize) -> Option<&[Option<Highlight>]> {
        self.lines.get(&(version, line)).map(Vec::as_slice)
    }

    /// Writes down the highlights `buffer` gives the lines of `version` that
    /// `shown` lists, each with how many characters it holds.
    fn paint(&mut self, version: Version, shown: &[(usize, usize)], buffer: &mut Buffer) {
        let (Some(first), Some(last)) = (
            shown.iter().map(|(line, _)| *line).min(),
            shown.iter().map(|(line, _)| *line).max(),
        ) else {
            return;
        };
        let highlights = buffer.highlights(first.saturating_sub(1)..last);
        for &(line, chars) in shown {
            let row = (0..chars)
                .map(|column| highlights.at(line.saturating_sub(1), column))
                .collect();
            self.lines.insert((version, line), row);
        }
    }
}

/// Every line `patch` shows, by the version it is read from, each with how
/// many characters it holds.
fn shown(patch: &Patch) -> BTreeMap<Version, Vec<(usize, usize)>> {
    let sides = [(true, &patch.staged), (false, &patch.unstaged)];
    let mut shown = BTreeMap::<Version, Vec<(usize, usize)>>::new();
    for (staged, hunks) in sides {
        for line in hunks.iter().flat_map(|hunk: &Hunk| &hunk.lines) {
            if let Some((version, number)) = Version::of(staged, line) {
                shown
                    .entry(version)
                    .or_default()
                    .push((number, line.text.chars().count()));
            }
        }
    }
    shown
}
