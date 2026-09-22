//! Where a file differs from what the index holds, hunk by hunk.
//!
//! The comparison is made here rather than asked of git, so the marks in the
//! gutter follow the typing: git is asked once for what the file used to
//! hold, and every keystroke after that is compared against that copy. Each
//! hunk carries what the index had, because putting a change back is exactly
//! writing that over the lines the hunk covers.

use std::ops::Range;

/// Longest run of differing lines that is compared line by line.
///
/// Beyond this the two sides are called one hunk rather than lined up
/// against each other: a table that size costs more to fill than the marks
/// in the gutter are worth, and a file rewritten wholesale reads as one
/// change anyway.
const ALIGNMENT_LIMIT: usize = 1000;

/// What happened to a run of lines in the working copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeKind {
    /// The lines are not in the index at all.
    Added,
    /// The lines are in the index, differently.
    Modified,
    /// Lines that were in the index are gone from here.
    Removed,
}

/// One run of lines that differs from what the index holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    /// The lines of the working copy it covers, empty when they were taken out.
    pub lines: Range<usize>,
    /// What the index held in their place.
    pub removed: String,
    /// What happened to them.
    pub kind: ChangeKind,
}

impl Change {
    /// The line the mark for this change is drawn against.
    pub fn anchor(&self) -> usize {
        match self.lines.is_empty() {
            true => self.lines.start.saturating_sub(1),
            false => self.lines.start,
        }
    }

    /// Whether `line` is one of the lines this change covers.
    pub fn covers(&self, line: usize) -> bool {
        self.lines.contains(&line) || (self.lines.is_empty() && self.anchor() == line)
    }
}

/// Where `after` differs from `before`.
///
/// The two are trimmed to the part that differs before anything is lined up,
/// which is what makes a one-character edit in a long file cost a scan of
/// the file rather than a comparison of it against itself.
pub fn changes(before: &str, after: &str) -> Vec<Change> {
    let old = before.lines().collect::<Vec<_>>();
    let new = after.lines().collect::<Vec<_>>();

    let head = common_prefix(&old, &new);
    let tail = common_suffix(&old[head..], &new[head..]);
    let old = &old[head..old.len() - tail];
    let new = &new[head..new.len() - tail];
    if old.is_empty() && new.is_empty() {
        return Vec::new();
    }

    if old.len() > ALIGNMENT_LIMIT || new.len() > ALIGNMENT_LIMIT {
        return vec![whole(head, old, new)];
    }
    aligned(head, old, new)
}

/// The one hunk two runs too long to line up come to.
fn whole(head: usize, old: &[&str], new: &[&str]) -> Change {
    Change {
        lines: head..head + new.len(),
        removed: joined(old),
        kind: kind(old.is_empty(), new.is_empty()),
    }
}

/// The hunks two runs come to, lined up against each other.
fn aligned(head: usize, old: &[&str], new: &[&str]) -> Vec<Change> {
    let kept = longest_common(old, new);
    let mut changes = Vec::new();
    let (mut taken, mut put) = (0, 0);

    for (from, to) in kept.iter().copied().chain([(old.len(), new.len())]) {
        if taken < from || put < to {
            changes.push(Change {
                lines: head + put..head + to,
                removed: joined(&old[taken..from]),
                kind: kind(taken == from, put == to),
            });
        }
        taken = from + 1;
        put = to + 1;
    }
    changes
}

/// Which kind a hunk is, given which of its two sides is empty.
fn kind(no_old: bool, no_new: bool) -> ChangeKind {
    match (no_old, no_new) {
        (true, _) => ChangeKind::Added,
        (_, true) => ChangeKind::Removed,
        _ => ChangeKind::Modified,
    }
}

/// `lines` back as one piece of text, each line ended.
fn joined(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| format!("{line}\n"))
        .collect::<String>()
}

/// How many lines the two agree on from the start.
fn common_prefix(old: &[&str], new: &[&str]) -> usize {
    old.iter().zip(new).take_while(|(a, b)| a == b).count()
}

/// How many lines the two agree on from the end.
fn common_suffix(old: &[&str], new: &[&str]) -> usize {
    old.iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
}

/// The pairs of lines the two runs have in common, in order.
///
/// This is the longest common subsequence, filled in as the table every
/// account of the problem begins with: the runs reaching here have already
/// had everything they agree on at either end taken off them, so the table
/// is as small as the edit was.
fn longest_common(old: &[&str], new: &[&str]) -> Vec<(usize, usize)> {
    let (rows, columns) = (old.len() + 1, new.len() + 1);
    let mut table = vec![0u32; rows * columns];

    for taken in (0..old.len()).rev() {
        for put in (0..new.len()).rev() {
            table[taken * columns + put] = if old[taken] == new[put] {
                table[(taken + 1) * columns + put + 1] + 1
            } else {
                table[(taken + 1) * columns + put].max(table[taken * columns + put + 1])
            };
        }
    }

    let mut pairs = Vec::new();
    let (mut taken, mut put) = (0, 0);
    while taken < old.len() && put < new.len() {
        if old[taken] == new[put] {
            pairs.push((taken, put));
            taken += 1;
            put += 1;
        } else if table[(taken + 1) * columns + put] >= table[taken * columns + put + 1] {
            taken += 1;
        } else {
            put += 1;
        }
    }
    pairs
}
