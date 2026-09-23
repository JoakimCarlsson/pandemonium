//! Typing: insert mode, where the window types and only the bound keys are
//! modal editing's, and replace mode, where every character types over the
//! one under the cursor.
//!
//! A stretch of typing begun by a command is one change: one undo step and
//! one thing for `.` to make again, keys and all.

use pm_text::{Buffer, Motion as Step, Position};

use crate::engine::cursors::{heads, place_all};
use crate::engine::{Stage, Typing, Vim};
use crate::key::{Key, Keystroke};
use crate::mode::Mode;
use crate::text::on_char;

impl Vim {
    /// A key typed in insert mode: a bound one does what it is bound to,
    /// anything else is the window's to type.
    ///
    /// Only keys being replayed are typed here, since then there is no
    /// window behind them.
    pub(crate) fn insert_key(&mut self, stage: &mut Stage, key: Keystroke) -> bool {
        if let Some(waiting) = stage.state.pending.waiting {
            self.note_typed(stage, key);
            self.waiting_key(stage, waiting, key);
            return true;
        }
        stage.state.pending.keys.push(key);
        let situation = stage.state.situation();
        let lookup = self.keymap.lookup(&stage.state.pending.keys, &situation);
        if lookup.longer {
            return true;
        }
        let keys = std::mem::take(&mut stage.state.pending.keys);
        for key in &keys {
            self.note_typed(stage, *key);
        }
        if let Some(action) = lookup.exact {
            return self.dispatch(stage, action);
        }
        if self.replaying() {
            type_key(stage.buffer, key);
            return true;
        }
        false
    }

    /// A key typed in replace mode: it types over the character under every
    /// cursor, and Backspace puts back what it typed over.
    pub(crate) fn replace_key(&mut self, stage: &mut Stage, key: Keystroke) -> bool {
        let lookup = self.keymap.lookup(&[key], &stage.state.situation());
        if let Some(action) = lookup.exact {
            self.note_typed(stage, key);
            return self.dispatch(stage, action);
        }
        if key.ctrl {
            return false;
        }
        self.note_typed(stage, key);
        let Key::Char(ch) = key.key else {
            type_key(stage.buffer, key);
            return true;
        };
        let mut over = Vec::new();
        stage.buffer.at_each(|buffer| {
            let head = buffer.selection().head;
            let under = buffer.char_at(head);
            let end = Position::new(head.line, head.column + usize::from(under.is_some()));
            buffer.replace(head..end, &ch.to_string());
            over.push(under);
        });
        if let (Some(typing), Some(under)) = (stage.state.typing.as_mut(), over.last()) {
            typing.replaced.push(*under);
        }
        true
    }

    /// Backspace in replace mode: puts back what the last character typed
    /// over stood on, or steps left past what was there before.
    pub(crate) fn undo_replace(&mut self, stage: &mut Stage) {
        let Some(typing) = stage.state.typing.as_mut() else {
            return;
        };
        let under = typing.replaced.pop();
        stage.buffer.at_each(|buffer| {
            let head = buffer.selection().head;
            let start = Position::new(head.line, head.column.saturating_sub(1));
            match under {
                Some(Some(over)) => {
                    buffer.replace(start..head, &over.to_string());
                    buffer.place(start, false);
                }
                Some(None) => buffer.backspace(),
                None => buffer.move_cursor(Step::Left, false),
            }
        });
    }

    /// Writes down a key typed as part of the change being made.
    pub(crate) fn note_typed(&mut self, stage: &mut Stage, key: Keystroke) {
        if !self.repeating
            && let Some(change) = self.change.as_mut()
        {
            change.keys.push(key);
        }
        if let Some(typing) = stage.state.typing.as_mut()
            && !key.is_escape()
        {
            typing.keys.push(key);
        }
    }

    /// Enters `mode` to type, joining what is typed onto the undo step that
    /// began at `depth`, and making it `count` times in all.
    pub(crate) fn begin_typing(
        &mut self,
        stage: &mut Stage,
        mode: Mode,
        depth: usize,
        count: usize,
        opens: Option<bool>,
    ) {
        stage.state.mode = mode;
        stage.state.regions.clear();
        stage.state.shown = None;
        stage.state.typing = Some(Typing {
            depth,
            count,
            opens,
            ..Typing::default()
        });
    }

    /// Ends the typing under way: makes it as many times as counted, joins
    /// it into one undo step and, when `step_back`, steps every cursor back
    /// onto the last character typed.
    pub(crate) fn finish_typing(&mut self, stage: &mut Stage, step_back: bool) {
        let buffer = &mut *stage.buffer;
        let mut collapse = false;
        if let Some(typing) = stage.state.typing.take() {
            for _ in 1..typing.count {
                match typing.opens {
                    Some(true) => buffer.at_each(Buffer::insert_line_below),
                    Some(false) => buffer.at_each(Buffer::insert_line_above),
                    None => {}
                }
                for key in &typing.keys {
                    type_key(buffer, *key);
                }
            }
            buffer.squash_since(typing.depth);
            collapse = typing.collapse;
            let typed = typing
                .keys
                .iter()
                .filter_map(|key| key.char())
                .collect::<String>();
            self.keep_typed(typed);
        }
        let (places, primary) = heads(buffer);
        stage
            .state
            .marks
            .insert('^', places.get(primary).copied().unwrap_or_default());
        let places = match collapse {
            true => places.into_iter().take(1).collect(),
            false => places,
        };
        let primary = if collapse { 0 } else { primary };
        let settled = places
            .into_iter()
            .map(|head| {
                let back = match step_back {
                    true => Position::new(head.line, head.column.saturating_sub(1)),
                    false => head,
                };
                on_char(buffer, back)
            })
            .collect();
        place_all(buffer, settled, primary);
        stage.state.mode = Mode::Normal;
        stage.state.goal = None;
    }
}

/// Types `key` at every cursor the way the window would have, for keys that
/// are replayed with no window behind them.
pub(crate) fn type_key(buffer: &mut Buffer, key: Keystroke) {
    if key.ctrl {
        return;
    }
    buffer.at_each(|buffer| match key.key {
        Key::Char(ch) => buffer.insert_typed(ch),
        Key::Enter => buffer.insert_newline(),
        Key::Backspace => buffer.backspace(),
        Key::Delete => buffer.delete(),
        Key::Tab => buffer.insert_indent(),
        Key::Left => buffer.move_cursor(Step::Left, false),
        Key::Right => buffer.move_cursor(Step::Right, false),
        Key::Up => buffer.move_cursor(Step::Up, false),
        Key::Down => buffer.move_cursor(Step::Down, false),
        Key::Home => buffer.move_cursor(Step::LineStart, false),
        Key::End => buffer.move_cursor(Step::LineEnd, false),
        Key::Escape | Key::PageUp | Key::PageDown | Key::Insert => {}
    });
}
