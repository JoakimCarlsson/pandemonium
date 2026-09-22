//! What one file's change looks like line by line, as git lays it out.
//!
//! This is the diff a reviewer reads, which is not the same thing as the
//! marks in a gutter: a review shows both sides of a line, the lines around
//! it and the side of the index it is on, while a gutter only has to know
//! that something happened here. [`crate::changes`] is the gutter's; this is
//! the screen's, and it is git's own answer rather than ours so that a
//! renamed, deleted or staged file reads the way git reads it.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::git::run::{within, written};

/// How many unchanged lines are shown either side of a change.
const CONTEXT: usize = 3;

/// The side git is given for a file that is being compared against nothing.
const NOTHING: &str = "/dev/null";

/// How git begins each file of a patch that holds several.
const HEADING: &str = "diff --git ";

/// What git wraps a path in when the path holds something unusual.
const QUOTE: char = '"';

/// Which comparison a diff is of.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Side {
    /// The index against the last commit: what a commit would hold.
    Staged,
    /// The worktree against the index: what a commit would leave behind.
    Unstaged,
    /// The whole of a file git has never been told about.
    Untracked,
}

/// What happened to one line of a file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineKind {
    /// The line is on both sides, and is shown for its surroundings.
    Context,
    /// The line is only on the new side.
    Added,
    /// The line is only on the old side.
    Removed,
}

/// One line of a diff: what happened to it, and where it sits on each side.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Line {
    /// What happened to it.
    pub kind: LineKind,
    /// The line itself, without the character that says what happened.
    pub text: String,
    /// Which line of the old side it is, when it is on the old side.
    pub old: Option<usize>,
    /// Which line of the new side it is, when it is on the new side.
    pub new: Option<usize>,
}

/// One run of lines a diff shows, with the lines around it.
///
/// The two sides are both written down, because a hunk is a replacement: the
/// run of lines it covers on the old side becomes the run it holds on the
/// new one. That is what staging one hunk comes to — the old side's lines,
/// in the text the index holds, written over with the new side's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hunk {
    /// What git wrote after the line numbers, which is usually the function.
    pub heading: String,
    /// The first line of the old side this hunk covers, counted from one.
    pub old_start: usize,
    /// How many lines of the old side it covers.
    pub old_count: usize,
    /// The first line of the new side this hunk covers, counted from one.
    pub start: usize,
    /// How many lines of the new side it covers.
    pub new_count: usize,
    /// The lines themselves, in the order they are read.
    pub lines: Vec<Line>,
}

impl Hunk {
    /// The lines this hunk holds on one side, as text with its line endings.
    ///
    /// A line that is only on the other side is left out, which is what makes
    /// this the side's own text rather than the diff's.
    pub fn side(&self, new: bool) -> String {
        let left_out = match new {
            true => LineKind::Removed,
            false => LineKind::Added,
        };
        self.lines
            .iter()
            .filter(|line| line.kind != left_out)
            .map(|line| format!("{}\n", line.text))
            .collect()
    }
}

impl Hunk {
    /// How many lines this hunk adds.
    pub fn added(&self) -> usize {
        self.count(LineKind::Added)
    }

    /// How many lines this hunk takes out.
    pub fn removed(&self) -> usize {
        self.count(LineKind::Removed)
    }

    /// How many of its lines are of `kind`.
    fn count(&self, kind: LineKind) -> usize {
        self.lines.iter().filter(|line| line.kind == kind).count()
    }
}

/// How `path` differs on `side`, in the worktree at `root`.
///
/// A file git will not diff — one outside the worktree, one whose side of
/// the index holds nothing — comes back with nothing to show rather than as
/// an error, because a diff with no hunks and a file that cannot be diffed
/// read the same on the screen.
pub fn diff(root: &Path, path: &Path, side: Side) -> Vec<Hunk> {
    let Some(relative) = within(root, path) else {
        return Vec::new();
    };
    let context = format!("-U{CONTEXT}");
    let mut arguments: Vec<&OsStr> = vec![
        OsStr::new("diff"),
        OsStr::new("--no-color"),
        OsStr::new("--no-ext-diff"),
        OsStr::new(&context),
    ];
    match side {
        Side::Staged => arguments.push(OsStr::new("--cached")),
        Side::Unstaged => {}
        Side::Untracked => arguments.push(OsStr::new("--no-index")),
    }
    arguments.push(OsStr::new("--"));
    if side == Side::Untracked {
        arguments.push(OsStr::new(NOTHING));
    }
    arguments.push(relative.as_os_str());

    read(&written(root, arguments))
}

/// How every file of the worktree at `root` differs on `side`.
///
/// A review reads the whole worktree, and the whole worktree is one question
/// to git: asking file by file would be a subprocess per row of a list that
/// is as long as the change is. A file git has never been told about is not
/// in either comparison and is not here — it has no old side to differ from,
/// and [`diff`] with [`Side::Untracked`] is what reads one of those.
pub fn diffs(root: &Path, side: Side) -> HashMap<PathBuf, Vec<Hunk>> {
    let context = format!("-U{CONTEXT}");
    let mut arguments: Vec<&OsStr> = vec![
        OsStr::new("diff"),
        OsStr::new("--no-color"),
        OsStr::new("--no-ext-diff"),
        OsStr::new(&context),
    ];
    if side == Side::Staged {
        arguments.push(OsStr::new("--cached"));
    }

    let text = written(root, arguments);
    let mut files = HashMap::new();
    for (path, patch) in split(&text) {
        files.insert(root.join(path), read(patch));
    }
    files
}

/// Each file of a patch of several, as the path it is to and its own patch.
fn split(text: &str) -> Vec<(PathBuf, &str)> {
    let mut files: Vec<(PathBuf, &str)> = Vec::new();
    let mut named: Option<PathBuf> = None;
    let mut from = 0;

    for (at, line) in offsets(text) {
        if line.starts_with(HEADING) {
            if let Some(path) = named.take() {
                files.push((path, &text[from..at]));
            }
            from = at;
        }
        if let Some(path) = sided(line) {
            named = named.or(Some(path));
        }
    }
    if let Some(path) = named {
        files.push((path, &text[from..]));
    }
    files
}

/// Each line of `text` and how far into it the line begins.
fn offsets(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().scan(0, |at, line| {
        let begins = *at;
        *at += line.len() + 1;
        Some((begins, line))
    })
}

/// The file a `+++` or `---` line names, when it names one.
///
/// The new side is what a file is listed under, and the old side is what a
/// file that has been deleted is listed under instead, because its new side
/// is nothing at all. Whichever comes first is the one taken.
fn sided(line: &str) -> Option<PathBuf> {
    let named = line
        .strip_prefix("+++ b/")
        .or_else(|| line.strip_prefix("--- a/"))?;
    (named != NOTHING).then(|| PathBuf::from(named.trim_matches(QUOTE)))
}

/// The hunks of a unified diff, read out of what git wrote.
///
/// Everything before the first hunk heading is git naming the two sides,
/// which the screen already knows, and the one line git writes about a file
/// that ends without a newline belongs to neither side.
fn read(text: &str) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut old = 0;
    let mut new = 0;

    for line in text.lines() {
        if let Some((heading, sides)) = heading(line) {
            (old, new) = sides;
            hunks.push(heading);
            continue;
        }
        let (Some(hunk), Some((mark, text))) = (hunks.last_mut(), line.split_at_checked(1)) else {
            continue;
        };
        let kind = match mark {
            " " => LineKind::Context,
            "+" => LineKind::Added,
            "-" => LineKind::Removed,
            _ => continue,
        };
        hunk.lines.push(Line {
            kind,
            text: text.to_owned(),
            old: (kind != LineKind::Added).then_some(old),
            new: (kind != LineKind::Removed).then_some(new),
        });
        if kind != LineKind::Added {
            old += 1;
        }
        if kind != LineKind::Removed {
            new += 1;
        }
    }
    hunks
}

/// The hunk `line` heads and the line each side starts at, if it heads one.
///
/// A heading reads `@@ -12,7 +12,9 @@ fn name`: a range per side, each a
/// first line and a count, and then whatever git worked out this hunk is
/// inside of.
fn heading(line: &str) -> Option<(Hunk, (usize, usize))> {
    let (ranges, heading) = line.strip_prefix("@@ ")?.split_once(" @@")?;
    let mut sides = ranges.split_whitespace().map(span);
    let (old, old_count) = sides.next()??;
    let (new, new_count) = sides.next()??;

    Some((
        Hunk {
            heading: heading.trim().to_owned(),
            old_start: old,
            old_count,
            start: new,
            new_count,
            lines: Vec::new(),
        },
        (old, new),
    ))
}

/// The first line and the count one side of a heading names.
///
/// A side with no count covers one line, which is how git writes a hunk that
/// is a single line: `-12` rather than `-12,1`.
fn span(side: &str) -> Option<(usize, usize)> {
    let mut numbers = side.get(1..)?.split(',');
    let first = numbers.next()?.parse().ok()?;
    let count = match numbers.next() {
        Some(count) => count.parse().ok()?,
        None => 1,
    };
    Some((first, count))
}
