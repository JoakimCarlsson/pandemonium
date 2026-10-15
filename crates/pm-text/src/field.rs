//! A single line of editable text, the caret in it and what is selected.

use std::ops::Range;

use crate::word::Class;

/// Where a caret moves to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motion {
    /// One character left.
    Left,
    /// One character right.
    Right,
    /// To the start of the word before the caret.
    WordLeft,
    /// To the end of the word after the caret.
    WordRight,
    /// To the start of the line.
    Home,
    /// To the end of the line.
    End,
}

/// One change to make to a [`Field`].
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// Type a character, replacing what is selected.
    Insert(char),
    /// Insert text, replacing what is selected; control characters are dropped.
    Paste(String),
    /// Delete what is selected, or else the character before the caret.
    Backspace,
    /// Delete what is selected, or else the character after the caret.
    Delete,
    /// Delete what is selected, or else the word before the caret.
    BackspaceWord,
    /// Delete what is selected, or else the word after the caret.
    DeleteWord,
    /// Move the caret, extending the selection when the flag is set and
    /// dropping it otherwise.
    Move(Motion, bool),
    /// Select the whole line.
    SelectAll,
    /// Put the caret at a byte offset of what is shown, extending the
    /// selection when the flag is set and dropping it otherwise.
    Place(usize, bool),
    /// Select the word around a byte offset of what is shown.
    SelectWordAt(usize),
    /// Take back the last change.
    Undo,
    /// Make again the change that was taken back.
    Redo,
    /// Scroll the line so it starts this many logical pixels left of the
    /// view; negative offsets are taken as none.
    ScrollTo(f32),
    /// Show text an input method is still composing at the caret, such as
    /// the accent of a dead key waiting for its letter, or stop showing it
    /// when empty. The text is not typed until the input method commits it.
    Compose(String),
}

/// Text with a caret between two of its characters and, when the
/// reader has selected some, the other end of the selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Field {
    /// The text being edited, which holds no control characters but line breaks and tabs, and those only in a multiline field.
    text: String,
    /// The caret, as a byte offset on a character boundary.
    caret: usize,
    /// The end of the selection that does not move, as a byte offset on a
    /// character boundary; equal to the caret when nothing is selected.
    anchor: usize,
    /// How far the line is scrolled left, in logical pixels.
    scroll: f32,
    /// How many characters the line may hold, if it is limited.
    limit: Option<usize>,
    /// Whether the line is shown as dots and cannot be copied from.
    masked: bool,
    /// Whether the line can be selected and copied but not changed.
    read_only: bool,
    /// Whether the text may hold line breaks, and so is more than one line.
    multiline: bool,
    /// The states the line was in before each change, oldest first.
    past: Vec<Snapshot>,
    /// The states that were taken back, most recently taken back last.
    future: Vec<Snapshot>,
    /// Whether the last change was typing, which the next typing joins.
    typing: bool,
    /// Text an input method is composing at the caret: shown, but not part
    /// of the text until it is committed.
    composing: String,
}

/// What a field held at one moment, for undoing back to it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    /// The line.
    text: String,
    /// The caret.
    caret: usize,
    /// The other end of the selection.
    anchor: usize,
}

/// What a masked line shows for each of its characters.
const MASK: char = '•';

/// How many changes undo remembers.
const HISTORY: usize = 200;

impl Field {
    /// A field holding `text`, with the caret at its end and nothing selected.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            caret: text.len(),
            anchor: text.len(),
            text,
            ..Self::default()
        }
    }

    /// The field limited to `characters` characters.
    pub fn limited(mut self, characters: usize) -> Self {
        self.limit = Some(characters);
        self
    }

    /// The field shown as dots, whose text cannot be copied.
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// The field that can be selected and copied from but not changed.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// The field that may hold line breaks: Enter's partner, a line break typed or pasted, is kept.
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    /// Whether the field may hold line breaks.
    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// Whether the field cannot be changed.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Whether there is a change to take back.
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    /// Whether there is a change that was taken back to make again.
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Whether the line can be copied from: something is selected and the line is not masked.
    pub fn can_copy(&self) -> bool {
        !self.selected().is_empty()
    }

    /// How far the line is scrolled left, in logical pixels.
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// What the line looks like: its text with what is being composed at the
    /// caret, or a dot for each character of that when masked.
    pub fn shown(&self) -> std::borrow::Cow<'_, str> {
        if self.composing.is_empty() && !self.masked {
            return std::borrow::Cow::Borrowed(&self.text);
        }
        let mut composed = self.text.clone();
        composed.insert_str(self.caret, &self.composing);
        if self.masked {
            std::borrow::Cow::Owned(MASK.to_string().repeat(composed.chars().count()))
        } else {
            std::borrow::Cow::Owned(composed)
        }
    }

    /// The offset in what is shown that matches the byte offset `byte` of the
    /// text; offsets from the caret on fall after what is being composed.
    pub fn to_shown(&self, byte: usize) -> usize {
        let after_composing = !self.composing.is_empty() && byte >= self.caret;
        self.to_shown_uncomposed(byte)
            + if after_composing {
                self.composing_width()
            } else {
                0
            }
    }

    /// The range of what is shown that an input method is still composing,
    /// or nothing while it composes nothing.
    pub fn marked(&self) -> Option<Range<usize>> {
        (!self.composing.is_empty()).then(|| {
            let start = self.to_shown_uncomposed(self.caret);
            start..start + self.composing_width()
        })
    }

    /// The offset in what is shown that matches the byte offset `byte` of the
    /// text, leaving out what is being composed.
    fn to_shown_uncomposed(&self, byte: usize) -> usize {
        if self.masked {
            self.text[..byte.min(self.text.len())].chars().count() * MASK.len_utf8()
        } else {
            byte
        }
    }

    /// How many bytes of what is shown the text being composed takes.
    fn composing_width(&self) -> usize {
        if self.masked {
            self.composing.chars().count() * MASK.len_utf8()
        } else {
            self.composing.len()
        }
    }

    /// The byte offset of the text that matches the offset `shown` in what is
    /// shown; an offset inside what is being composed is the caret's.
    fn to_text(&self, shown: usize) -> usize {
        let start = self.to_shown_uncomposed(self.caret);
        let width = self.composing_width();
        let shown = if shown >= start + width {
            shown - width
        } else {
            shown.min(start)
        };
        if !self.masked {
            return self.boundary(shown);
        }
        let characters = shown / MASK.len_utf8();
        self.text
            .char_indices()
            .nth(characters)
            .map_or(self.text.len(), |(at, _)| at)
    }

    /// Where the line should scroll to so the caret, `caret_x` logical pixels
    /// from the start of the line, stays inside a view `view` wide.
    pub fn scroll_for(&self, caret_x: f32, line: f32, view: f32) -> f32 {
        let margin = 2.0;
        let mut scroll = self.scroll;
        if caret_x - scroll > view - margin {
            scroll = caret_x - view + margin;
        }
        if caret_x - scroll < 0.0 {
            scroll = caret_x;
        }
        scroll.clamp(0.0, (line - view + margin).max(0.0))
    }

    /// The line as typed so far, without the caret.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret, as a byte offset into [`Field::text`].
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The selected bytes of [`Field::text`], or nothing when none are.
    pub fn selection(&self) -> Option<Range<usize>> {
        (self.anchor != self.caret)
            .then(|| self.anchor.min(self.caret)..self.anchor.max(self.caret))
    }

    /// The selected text, empty when nothing is selected or the line is masked.
    pub fn selected(&self) -> &str {
        match self.selection() {
            Some(range) if !self.masked => &self.text[range],
            _ => "",
        }
    }

    /// Makes `edit`.
    pub fn apply(&mut self, edit: Edit) {
        match &edit {
            Edit::Undo => return self.undo(),
            Edit::Redo => return self.redo(),
            Edit::ScrollTo(offset) => {
                self.scroll = offset.max(0.0);
                return;
            }
            Edit::Compose(text) => {
                self.composing = if self.read_only {
                    String::new()
                } else {
                    text.chars().filter(|c| !c.is_control()).collect()
                };
                return;
            }
            _ => {}
        }
        let changes = matches!(
            edit,
            Edit::Insert(_)
                | Edit::Paste(_)
                | Edit::Backspace
                | Edit::Delete
                | Edit::BackspaceWord
                | Edit::DeleteWord
        );
        if changes && self.read_only {
            return;
        }
        let before = changes.then(|| self.snapshot());
        let typed = matches!(&edit, Edit::Insert(c) if !c.is_whitespace());
        match edit {
            Edit::Insert(character) => {
                if !character.is_control() || (matches!(character, '\n' | '\t') && self.multiline) {
                    self.replace_selection(character.encode_utf8(&mut [0; 4]));
                }
            }
            Edit::Paste(text) => {
                let multiline = self.multiline;
                let clean: String = text
                    .replace("\r\n", "\n")
                    .chars()
                    .filter(|c| !c.is_control() || (multiline && matches!(*c, '\n' | '\t')))
                    .collect();
                self.replace_selection(&clean);
            }
            Edit::Backspace => self.delete_or(|field| field.before(field.caret)),
            Edit::Delete => self.delete_or(|field| field.after(field.caret)),
            Edit::BackspaceWord => self.delete_or(|field| Some(field.word_before(field.caret))),
            Edit::DeleteWord => self.delete_or(|field| Some(field.word_after(field.caret))),
            Edit::Move(motion, extend) => self.travel(motion, extend),
            Edit::SelectAll => {
                self.anchor = 0;
                self.caret = self.text.len();
            }
            Edit::Place(shown, extend) => {
                self.caret = self.to_text(shown);
                if !extend {
                    self.anchor = self.caret;
                }
            }
            Edit::SelectWordAt(shown) => {
                let byte = self.to_text(shown);
                self.anchor = self.word_before(self.after(byte).unwrap_or(byte));
                self.caret = self.word_after(self.before(byte).unwrap_or(byte));
            }
            Edit::Undo | Edit::Redo | Edit::ScrollTo(_) | Edit::Compose(_) => {}
        }
        if let Some(before) = before
            && before != self.snapshot()
        {
            if !(typed && self.typing) {
                self.past.push(before);
                if self.past.len() > HISTORY {
                    self.past.remove(0);
                }
            }
            self.future.clear();
            self.typing = typed;
        } else if !changes {
            self.typing = false;
        }
    }

    /// The line, caret and selection as they are now.
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            caret: self.caret,
            anchor: self.anchor,
        }
    }

    /// Takes in a state the line is put back to.
    fn restore(&mut self, snapshot: Snapshot) {
        self.text = snapshot.text;
        self.caret = snapshot.caret;
        self.anchor = snapshot.anchor;
        self.typing = false;
    }

    /// Takes back the last change.
    fn undo(&mut self) {
        if let Some(previous) = self.past.pop() {
            self.future.push(self.snapshot());
            self.restore(previous);
        }
    }

    /// Makes again the last change that was taken back.
    fn redo(&mut self) {
        if let Some(next) = self.future.pop() {
            self.past.push(self.snapshot());
            self.restore(next);
        }
    }

    /// Replaces what is selected, or else nothing at the caret, with `text`.
    fn replace_selection(&mut self, text: &str) {
        let range = self.selection().unwrap_or(self.caret..self.caret);
        let kept = self.text.chars().count() - self.text[range.clone()].chars().count();
        let room = self
            .limit
            .map_or(usize::MAX, |limit| limit.saturating_sub(kept));
        let text: String = text.chars().take(room).collect();
        self.caret = range.start + text.len();
        self.text.replace_range(range, &text);
        self.anchor = self.caret;
    }

    /// Deletes what is selected, or else what lies between the caret and the
    /// offset `other` finds.
    fn delete_or(&mut self, other: impl Fn(&Self) -> Option<usize>) {
        let range = self.selection().or_else(|| {
            let other = other(self)?;
            Some(other.min(self.caret)..other.max(self.caret))
        });
        if let Some(range) = range {
            self.caret = range.start;
            self.text.replace_range(range, "");
            self.anchor = self.caret;
        }
    }

    /// Moves the caret by `motion`.
    fn travel(&mut self, motion: Motion, extend: bool) {
        let selection = self.selection();
        let target = match (motion, &selection) {
            (Motion::Left, Some(range)) if !extend => range.start,
            (Motion::Right, Some(range)) if !extend => range.end,
            (Motion::Left, _) => self.before(self.caret).unwrap_or(self.caret),
            (Motion::Right, _) => self.after(self.caret).unwrap_or(self.caret),
            (Motion::WordLeft, _) => self.word_before(self.caret),
            (Motion::WordRight, _) => self.word_after(self.caret),
            (Motion::Home, _) => 0,
            (Motion::End, _) => self.text.len(),
        };
        self.caret = target;
        if !extend {
            self.anchor = target;
        }
    }

    /// The nearest character boundary at or before `byte`, within the text.
    fn boundary(&self, byte: usize) -> usize {
        let mut byte = byte.min(self.text.len());
        while !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        byte
    }

    /// The offset of the character boundary before `from`.
    fn before(&self, from: usize) -> Option<usize> {
        self.text[..from]
            .char_indices()
            .next_back()
            .map(|(start, _)| start)
    }

    /// The offset of the character boundary after `from`.
    fn after(&self, from: usize) -> Option<usize> {
        self.text[from..]
            .chars()
            .next()
            .map(|character| from + character.len_utf8())
    }

    /// The start of the word before `from`: past any space, then past the
    /// run of characters of one class.
    fn word_before(&self, from: usize) -> usize {
        let mut at = from;
        let mut class = None;
        for (start, character) in self.text[..from].char_indices().rev() {
            let this = Class::of(character);
            match class {
                None if this == Class::Space => {}
                None => class = Some(this),
                Some(class) if class != this => break,
                Some(_) => {}
            }
            at = start;
        }
        at
    }

    /// The end of the word after `from`: past any space, then past the run
    /// of characters of one class.
    fn word_after(&self, from: usize) -> usize {
        let mut at = from;
        let mut class = None;
        for (start, character) in self.text[from..].char_indices() {
            let this = Class::of(character);
            match class {
                None if this == Class::Space => {}
                None => class = Some(this),
                Some(class) if class != this => break,
                Some(_) => {}
            }
            at = from + start + character.len_utf8();
        }
        at
    }
}
