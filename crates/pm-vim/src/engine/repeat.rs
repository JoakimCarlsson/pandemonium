//! Doing a change again: `.` for the last change, `@` for a recorded
//! register.
//!
//! A change is kept as the keys that made it, counts left out and kept
//! apart, so that `3.` makes it with a count of three whatever count it was
//! first made with; one made to a selection keeps the selection's shape, so
//! that `.` selects as much again from the cursor before making it.

use crate::engine::{Stage, Vim, visual};
use crate::key::Keystroke;
use crate::mode::Mode;

/// How much a selection covered, for `.` to select as much again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VisualShape {
    /// The kind of selection it was.
    pub mode: Mode,
    /// How many lines below the first it reached.
    pub lines: usize,
    /// How far it reached along its last line, or across for a block.
    pub width: usize,
    /// Whether a block ran to the end of every line.
    pub to_end: bool,
}

/// A change, as `.` makes it again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Change {
    /// The keys that made it, without their counts.
    pub keys: Vec<Keystroke>,
    /// The count it was made with.
    pub count: Option<usize>,
    /// The selection it was made to, when it was made in visual mode.
    pub visual: Option<VisualShape>,
}

impl Vim {
    /// Makes the last change again, with the count given now when there is
    /// one.
    pub(crate) fn repeat(&mut self, stage: &mut Stage) {
        let Some(change) = self.last_change.clone() else {
            return;
        };
        let count = stage.state.pending.count().or(change.count);
        stage.state.pending = Default::default();
        if let Some(shape) = change.visual {
            let at = stage.buffer.selection().head;
            visual::select_shape(stage.state, stage.buffer, shape, at);
        }
        stage.state.pending.pre = count;
        self.repeating = true;
        for key in change.keys {
            self.feed(stage, key);
        }
        self.repeating = false;
    }

    /// Plays the keys recorded in register `name`, `times` over.
    pub(crate) fn play(&mut self, stage: &mut Stage, name: char, times: usize) {
        let name = name.to_ascii_lowercase();
        let Some(keys) = self.macros.get(&name).cloned() else {
            return;
        };
        self.last_macro = Some(name);
        stage.state.pending = Default::default();
        self.playing += 1;
        for _ in 0..times {
            for key in &keys {
                self.feed(stage, *key);
            }
        }
        self.playing -= 1;
    }
}
