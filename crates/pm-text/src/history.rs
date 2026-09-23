//! What the text has been through, and how to take it back.
//!
//! Every replacement a buffer makes is written down here before it is made,
//! so undo is a replacement of its own rather than a copy of the file kept
//! to one side. Consecutive keystrokes are one step: typing a word and
//! taking it back are one gesture each way, which is what the grouping
//! interval and the kind of a change are for.

use std::time::{Duration, Instant};

use crate::cursor::{Position, Selection};

/// Longest pause between two changes that still makes them one step.
const GROUPING: Duration = Duration::from_millis(500);

/// What a change did, for deciding whether the next one continues it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    /// Text was put in and nothing taken out.
    Insert,
    /// Text was taken out and nothing put in.
    Delete,
    /// A replacement, which never joins the step before it.
    Other,
}

impl Kind {
    /// Which kind replacing `before` with `after` is.
    fn of(before: &str, after: &str) -> Self {
        match (before.is_empty(), after.is_empty()) {
            (true, false) => Self::Insert,
            (false, true) => Self::Delete,
            _ => Self::Other,
        }
    }

    /// Whether a change of this kind may join the step before it.
    fn groups(self) -> bool {
        self != Self::Other
    }
}

/// One replacement of a span of text with another.
#[derive(Clone, Debug)]
pub struct Change {
    /// Where the span began.
    pub at: Position,
    /// What was there before.
    pub before: String,
    /// What is there instead.
    pub after: String,
}

impl Change {
    /// The change that undoes this one.
    fn inverted(&self) -> Self {
        Self {
            at: self.at,
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }

    /// The end of the span this change leaves behind.
    fn end(&self) -> Position {
        self.at.after(&self.after)
    }

    /// Whether `next` carries on from this change without a gap.
    ///
    /// Typing carries on where the last character was left; backspacing
    /// carries on where the last one was taken from, and the delete key
    /// takes the next character from the same place every time.
    fn continues(&self, next: &Self, kind: Kind) -> bool {
        match kind {
            Kind::Insert => next.at == self.end(),
            Kind::Delete => next.at.after(&next.before) == self.at || next.at == self.at,
            Kind::Other => false,
        }
    }
}

/// One step of undo: the changes it covers and where the cursor was.
#[derive(Clone, Debug)]
struct Step {
    /// The changes, in the order they were made.
    changes: Vec<Change>,
    /// What was selected before the first of them.
    before: Selection,
    /// What was selected after the last of them.
    after: Selection,
    /// When the last of them was recorded.
    at: Instant,
    /// What kind of change they all were.
    kind: Kind,
}

/// What a buffer has been through: what can be taken back, and what put back.
#[derive(Default)]
pub struct History {
    /// The steps that have been made, oldest first.
    done: Vec<Step>,
    /// The steps that have been taken back, oldest first.
    undone: Vec<Step>,
    /// Whether the last step may still take further changes.
    open: bool,
    /// How many callers deep the gathering of changes into one step goes.
    gathering: usize,
    /// Whether that step has been started yet.
    gathered: bool,
}

/// A step to apply to the text, and where it leaves the cursor.
pub struct Replay {
    /// The changes to make, in the order to make them.
    pub changes: Vec<Change>,
    /// What is selected once they have been made.
    pub selection: Selection,
}

impl History {
    /// Writes `change` down, joining the step before it when it carries on.
    ///
    /// Recording anything at all discards what had been taken back: the
    /// history is a line, not a tree, which is what every editor's redo
    /// means by being lost.
    pub fn record(&mut self, change: Change, before: Selection, after: Selection) {
        self.undone.clear();
        let kind = Kind::of(&change.before, &change.after);

        if self.gathering > 0
            && self.gathered
            && let Some(step) = self.done.last_mut()
        {
            step.changes.push(change);
            step.after = after;
            step.kind = Kind::Other;
            return;
        }

        if self.open
            && kind.groups()
            && let Some(step) = self.done.last_mut()
            && step.kind == kind
            && step.at.elapsed() < GROUPING
            && step
                .changes
                .last()
                .is_some_and(|last| last.continues(&change, kind))
        {
            step.changes.push(change);
            step.after = after;
            step.at = Instant::now();
            return;
        }

        self.done.push(Step {
            changes: vec![change],
            before,
            after,
            at: Instant::now(),
            kind,
        });
        self.open = true;
        self.gathered = self.gathering > 0;
    }

    /// Gathers every change recorded from here into one step.
    ///
    /// An operation that rewrites several lines at once is one thing the
    /// reader asked for, and one thing undo should take back, however many
    /// replacements it took to carry out. Gathering nests: one such
    /// operation carried out at every cursor is still one step.
    pub fn begin(&mut self) {
        if self.gathering == 0 {
            self.commit();
            self.gathered = false;
        }
        self.gathering += 1;
    }

    /// Ends the step [`Self::begin`] opened.
    pub fn end(&mut self) {
        self.gathering = self.gathering.saturating_sub(1);
        if self.gathering == 0 {
            self.gathered = false;
            self.commit();
        }
    }

    /// Ends the step being written, so the next change starts its own.
    pub fn commit(&mut self) {
        self.open = false;
    }

    /// The changes that take the last step back, if there is one.
    pub fn undo(&mut self) -> Option<Replay> {
        let step = self.done.pop()?;
        let replay = Replay {
            changes: step.changes.iter().rev().map(Change::inverted).collect(),
            selection: step.before,
        };
        self.undone.push(step);
        self.open = false;
        Some(replay)
    }

    /// The changes that put the last step back, if one was taken back.
    pub fn redo(&mut self) -> Option<Replay> {
        let step = self.undone.pop()?;
        let replay = Replay {
            changes: step.changes.clone(),
            selection: step.after,
        };
        self.done.push(step);
        self.open = false;
        Some(replay)
    }

    /// Makes every step after the first `depth` one step.
    ///
    /// Nothing happens when fewer than two steps have been made since, as
    /// there is nothing to join.
    pub fn squash(&mut self, depth: usize) {
        if self.done.len() <= depth + 1 {
            return;
        }
        let steps = self.done.split_off(depth);
        let before = steps[0].before;
        let last = &steps[steps.len() - 1];
        let (after, at) = (last.after, last.at);
        let changes = steps.into_iter().flat_map(|step| step.changes).collect();
        self.done.push(Step {
            changes,
            before,
            after,
            at,
            kind: Kind::Other,
        });
        self.open = false;
    }

    /// Whether there is a step to take back.
    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    /// Whether there is a step to put back.
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// How many steps have been made, for telling what is on disk apart.
    pub fn depth(&self) -> usize {
        self.done.len()
    }
}
