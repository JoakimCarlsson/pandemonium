//! What colour every character of a diff is drawn in.
//!
//! A diff shows lines out of three texts — the last commit, the index and
//! the worktree — and each line is coloured as it reads in the text it came
//! from, parsed whole, because a line of a hunk on its own is a fragment no
//! grammar can read. The worktree's lines are coloured again from the file's
//! open document once a language server has said what its names are, so a
//! change reads the way the same file does in an editor pane.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use pm_core::{Hunk, Line, LineKind, Revision};
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

    /// The revision git holds this version in, which the worktree is not.
    fn revision(self) -> Option<Revision> {
        match self {
            Self::Committed => Some(Revision::Head),
            Self::Indexed => Some(Revision::Index),
            Self::Worktree => None,
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
    /// Reads every file of `patches` in every version its patch shows a
    /// line of, in the worktree at `root`, and colours those lines.
    ///
    /// What the commit and the index hold is asked of git for every file at
    /// once, because a subprocess per file and version is what made staging
    /// two thousand files leave the review unread for a quarter of a minute.
    pub fn all(root: &Path, patches: &BTreeMap<PathBuf, Patch>) -> BTreeMap<PathBuf, Self> {
        let shown = patches
            .iter()
            .map(|(path, patch)| (path.as_path(), shown(patch)))
            .collect::<Vec<_>>();
        let wanted = shown
            .iter()
            .flat_map(|(path, versions)| {
                versions
                    .keys()
                    .filter_map(|version| version.revision())
                    .map(move |revision| (*path, revision))
            })
            .collect::<Vec<_>>();
        let mut held = wanted
            .iter()
            .copied()
            .zip(pm_core::contents(root, &wanted))
            .filter_map(|(asked, text)| Some((asked, text?)))
            .collect::<HashMap<_, _>>();

        shown
            .into_iter()
            .map(|(path, versions)| {
                let mut shading = Self::default();
                for (version, lines) in versions {
                    let text = match version.revision() {
                        Some(revision) => held.remove(&(path, revision)),
                        None => std::fs::read_to_string(path).ok(),
                    };
                    let Some(text) = text else {
                        continue;
                    };
                    let mut buffer = Buffer::holding(path, &text);
                    shading.paint(version, &lines, &mut buffer);
                }
                (path.to_path_buf(), shading)
            })
            .collect()
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
