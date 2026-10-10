//! One box of text: what is in it, and what the keyboard does to it.
//!
//! The text is a buffer, the same one the editor is made of, so an input
//! takes what the editor takes: selection, several cursors, undo, the word
//! and line motions. What an input adds is the two things a box has and a
//! document does not — how many lines it may hold, and what finishes it.

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use pm_gfx::Point;
use pm_text::Position;
use pm_ui::ResizePhase;
use winit::event::KeyEvent;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::editor::{self, Document, OpenFile};
use crate::keymap::Action;

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
    text: OnceCell<OpenFile>,
    /// The name shown when the input buffer is first created.
    name: String,
    /// How many lines it may hold.
    lines: Lines,
    /// What finishes it.
    submit: Submit,
}

impl Input {
    /// A box of one line, sent with Enter, called `name` where a name shows.
    pub fn one_line(name: &str) -> Self {
        Self {
            text: OnceCell::new(),
            name: name.to_owned(),
            lines: Lines::One,
            submit: Submit::Enter,
        }
    }

    /// A box of as many lines as are typed, sent with Enter, a line too
    /// long for the box carrying on down the next row.
    pub fn many_lines(name: &str) -> Self {
        Self {
            text: OnceCell::new(),
            name: name.to_owned(),
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
        self.document().clone()
    }

    /// What is in the box.
    pub fn value(&self) -> String {
        self.text
            .get()
            .map_or_else(String::new, |text| text.borrow().buffer().contents())
    }

    /// Whether there is nothing in it.
    pub fn is_empty(&self) -> bool {
        self.value().is_empty()
    }

    /// How many rows its text comes to, as wide as the box was last drawn.
    pub fn rows(&self) -> usize {
        self.document().borrow().total_rows()
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
        let value = match self.lines {
            Lines::One => value.lines().next().unwrap_or(""),
            Lines::Many => value,
        };
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

    /// Applies `event` to the box, saying whether it was one the box wanted.
    ///
    /// A box of one line has no line to break: Enter that does not finish it
    /// does nothing rather than growing a box the screen has no room for.
    pub fn press(&mut self, event: &KeyEvent, modifiers: ModifiersState) -> Typed {
        let rows = self.document().borrow().rows();
        let Some(edit) = editor::edit(event, modifiers, rows) else {
            return Typed::Ignored;
        };
        if self.lines == Lines::One
            && matches!(
                edit,
                editor::Edit::Newline | editor::Edit::Indent | editor::Edit::Outdent
            )
        {
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
        self.document()
            .borrow()
            .layout()
            .text_area()
            .contains(point)
    }

    /// Scrolls the box `pixels` down, or up when `pixels` is negative.
    ///
    /// A box stops with its last line at its foot, not at its head the way
    /// a file does: past that there is only the empty box to look at.
    pub fn scroll_by(&self, pixels: f32) {
        self.document().borrow_mut().scroll_by_pixels(pixels);
        self.keep_foot();
    }

    /// How many rows of its text are above the first one the box shows.
    pub fn rows_above(&self) -> usize {
        self.document().borrow().rows_above()
    }

    /// Shows the box from the row `rows` into its text down, no deeper than
    /// its last line at its foot.
    pub fn scroll_to_row(&self, rows: usize) {
        self.document().borrow_mut().scroll_to_wrapped(rows);
        self.keep_foot();
    }

    /// Brings a box scrolled past its last line back to that line at its foot.
    fn keep_foot(&self) {
        let mut text = self.document().borrow_mut();
        let deepest = text.row_after(text.last_row(), 1 - text.rows().max(1) as isize);
        if text.top() >= deepest {
            text.scroll_to_row(deepest);
        }
    }

    /// Creates a single-line input containing `value`.
    pub fn filled(value: impl AsRef<str>) -> Self {
        let mut input = Self::default();
        input.set(value.as_ref());
        input
    }

    /// Places the caret at a character offset in this single-line input.
    pub fn place(&mut self, caret: usize) {
        self.edit(|buffer| buffer.place(Position::new(0, caret), false));
    }

    /// Selects all text in the input.
    pub fn select_all(&mut self) {
        self.edit(pm_text::Buffer::select_all);
    }

    /// Returns the selected text, if the selection is nonempty.
    pub fn selected_text(&self) -> Option<String> {
        let document = self.document().borrow();
        let selected = document.buffer().selected_text();
        (!selected.is_empty()).then_some(selected)
    }

    /// Removes and returns the selected text, if any.
    pub fn cut_selection(&mut self) -> Option<String> {
        let selected = self.selected_text()?;
        self.edit(pm_text::Buffer::delete);
        Some(selected)
    }

    /// Initializes the document on first use, keeping nested search inputs lazy.
    fn document(&self) -> &OpenFile {
        self.text.get_or_init(|| {
            let document = Document::scratch(&self.name);
            let document = match self.lines {
                Lines::One => document,
                Lines::Many => document.wrapped(),
            };
            Rc::new(RefCell::new(document))
        })
    }

    /// Whether the action edits input text, its selection or its clipboard.
    pub fn handles(action: Action) -> bool {
        matches!(
            action,
            Action::Cut
                | Action::Copy
                | Action::Paste
                | Action::SelectAll
                | Action::Undo
                | Action::Redo
                | Action::Move(_)
                | Action::Select(_)
                | Action::Backspace
                | Action::Delete
                | Action::DeleteWordLeft
                | Action::DeleteWordRight
                | Action::DeleteToLineStart
                | Action::DeleteToLineEnd
        )
    }

    /// Applies a named editing action, leaving clipboard access and submission to the caller.
    pub fn act(&mut self, action: Action) -> bool {
        match action {
            Action::Undo => self.edit(|buffer| {
                buffer.undo();
            }),
            Action::Redo => self.edit(|buffer| {
                buffer.redo();
            }),
            Action::SelectAll => self.select_all(),
            _ => {
                if !Self::handles(action) {
                    return false;
                }
                let rows = self.document().borrow().rows();
                let Some(edit) = editor::action_edit(action, rows) else {
                    return false;
                };
                self.edit(|buffer| buffer.at_each(|buffer| edit.apply(buffer)));
            }
        }
        true
    }

    /// Puts the buffer through `change`.
    pub fn edit(&mut self, change: impl FnOnce(&mut pm_text::Buffer)) {
        self.document().borrow_mut().edit(change);
    }
}

impl Default for Input {
    /// Creates an empty single-line input without allocating its document yet.
    fn default() -> Self {
        Self::one_line("Input")
    }
}

impl Clone for Input {
    /// Copies the input's text and selection into an independent buffer.
    fn clone(&self) -> Self {
        let mut input = Self {
            text: OnceCell::new(),
            name: self.name.clone(),
            lines: self.lines,
            submit: self.submit,
        };
        if let Some(text) = self.text.get() {
            let text = text.borrow();
            input.set(&text.buffer().contents());
            let selection = text.buffer().selection();
            input.edit(|buffer| {
                buffer.place(selection.anchor, false);
                buffer.place(selection.head, true);
            });
        }
        input
    }
}

impl std::fmt::Debug for Input {
    /// Describes the input's current text without exposing its document internals.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Input")
            .field("value", &self.value())
            .finish()
    }
}

/// What a keypress aimed at an input came to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Typed {
    /// The input took the key.
    Taken,
    /// The key means nothing to this input.
    Ignored,
}

/// Whether the macOS command key asks to edit the current line.
pub fn command_line(key: &Key<&str>, modifiers: ModifiersState) -> bool {
    cfg!(target_os = "macos")
        && modifiers.super_key()
        && !modifiers.control_key()
        && !modifiers.alt_key()
        && matches!(
            key,
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight | NamedKey::Backspace)
        )
}
