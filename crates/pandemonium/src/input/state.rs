//! One box of text: what is in it, and what the keyboard does to it.
//!
//! The text is a buffer, the same one the editor is made of, so an input
//! takes what the editor takes: selection, several cursors, undo, the word
//! and line motions. What an input adds is the two things a box has and a
//! document does not — how many lines it may hold, and what finishes it.

use std::cell::RefCell;
use std::rc::Rc;

use pm_gfx::Point;
use pm_text::Position;
use pm_ui::ResizePhase;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::editor::{self, Document, OpenFile};
use crate::field::Typed;

/// How much text a box holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lines {
    /// One line: a name, a query, a path.
    One,
    /// As many as are typed: a message, a prompt.
    Many,
}

/// What finishes a box.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Submit {
    /// Enter sends it, and Shift with Enter breaks the line instead.
    ///
    /// This is for a box whose whole purpose is to be sent — a prompt — where
    /// sending is what the reader does every time and a line break is the
    /// exception.
    Enter,
    /// The platform key with Enter sends it, and Enter breaks the line.
    ///
    /// This is for a box that is written before it is sent — a commit message
    /// — where a stray Enter should never be the thing that sends it.
    Chord,
}

/// One box of text being written in.
pub struct Input {
    /// The text itself, and everything the editor knows about it.
    text: OpenFile,
    /// How many lines it may hold.
    lines: Lines,
    /// What finishes it.
    submit: Submit,
}

impl Input {
    /// A box of one line, sent with Enter, called `name` where a name shows.
    pub fn one_line(name: &str) -> Self {
        Self {
            text: Rc::new(RefCell::new(Document::scratch(name))),
            lines: Lines::One,
            submit: Submit::Enter,
        }
    }

    /// A box of as many lines as are typed, sent with Enter, a line too
    /// long for the box carrying on down the next row.
    pub fn many_lines(name: &str) -> Self {
        Self {
            text: Rc::new(RefCell::new(Document::scratch(name).wrapped())),
            lines: Lines::Many,
            submit: Submit::Enter,
        }
    }

    /// Returns this box finished by `submit` rather than by plain Enter.
    pub fn submitting(mut self, submit: Submit) -> Self {
        self.submit = submit;
        self
    }

    /// The buffer behind the box, for the screen that draws it.
    pub fn text(&self) -> OpenFile {
        self.text.clone()
    }

    /// What is in the box.
    pub fn value(&self) -> String {
        self.text.borrow().buffer().contents()
    }

    /// Whether there is nothing in it.
    pub fn is_empty(&self) -> bool {
        self.value().is_empty()
    }

    /// Empties it.
    pub fn clear(&mut self) {
        self.edit(|buffer| {
            buffer.select_all();
            buffer.delete();
        });
    }

    /// Puts `value` in, leaving the cursor at the end of it.
    pub fn set(&mut self, value: &str) {
        self.clear();
        self.edit(|buffer| buffer.insert(value));
    }

    /// Pastes text at the current selection.
    pub fn paste(&mut self, value: &str) {
        self.edit(|buffer| buffer.at_each(|buffer| buffer.paste(value)));
    }

    /// Whether `key` is the one that finishes this box.
    pub fn submits(&self, key: &Key, modifiers: ModifiersState) -> bool {
        if !matches!(key, Key::Named(NamedKey::Enter)) {
            return false;
        }
        match self.submit {
            Submit::Enter => !modifiers.shift_key(),
            Submit::Chord => modifiers.super_key() || modifiers.control_key(),
        }
    }

    /// Applies `key` to the box, saying whether it was one the box wanted.
    ///
    /// A box of one line has no line to break: Enter that does not finish it
    /// does nothing rather than growing a box the screen has no room for.
    pub fn press(&mut self, key: &Key, modifiers: ModifiersState) -> Typed {
        let rows = self.text.borrow().rows();
        let Some(edit) = editor::edit(key, modifiers, rows) else {
            return Typed::Ignored;
        };
        if self.lines == Lines::One && matches!(edit, editor::Edit::Newline) {
            return Typed::Ignored;
        }

        self.edit(|buffer| buffer.at_each(|buffer| edit.apply(buffer)));
        Typed::Taken
    }

    /// Answers a press, a drag or a release of the pointer in the box.
    ///
    /// `presses` is how many times the pointer has been pressed in the same
    /// place, so that the box selects what every other box selects: a word on
    /// the second press, the line on the third. `extend` keeps the selection's
    /// anchor and moves its head to where the pointer is, which is what a
    /// click with shift held asks for.
    pub fn point(
        &mut self,
        phase: ResizePhase,
        anchor: Position,
        head: Position,
        presses: usize,
        extend: bool,
    ) {
        let still = anchor == head;
        if still && phase != ResizePhase::Started {
            return;
        }

        self.edit(|buffer| {
            buffer.collapse_cursors();
            if extend {
                buffer.place(head, true);
                return;
            }
            match (still, presses) {
                (true, 2) => buffer.select_word(head),
                (true, count) if count >= 3 => buffer.select_line_text(head),
                (true, _) => buffer.place(head, false),
                (false, _) => {
                    buffer.place(anchor, false);
                    buffer.place(head, true);
                }
            }
        });
    }

    /// Whether `point` falls on the text of the box, as it was last drawn.
    pub fn covers(&self, point: Point) -> bool {
        self.text.borrow().layout().text_area().contains(point)
    }

    /// Scrolls the box `pixels` down, or up when `pixels` is negative.
    ///
    /// A box stops with its last line at its foot, not at its head the way
    /// a file does: past that there is only the empty box to look at.
    pub fn scroll_by(&self, pixels: f32) {
        let mut text = self.text.borrow_mut();
        text.scroll_by_pixels(pixels);
        let deepest = text.row_after(text.last_row(), 1 - text.rows().max(1) as isize);
        if text.top() >= deepest {
            text.scroll_to_row(deepest);
        }
    }

    /// Puts the buffer through `change`.
    fn edit(&mut self, change: impl FnOnce(&mut pm_text::Buffer)) {
        self.text.borrow_mut().edit(change);
    }
}
