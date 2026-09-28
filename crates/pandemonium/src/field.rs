//! One line of text being typed into: what is in it, and where the caret is.
//!
//! A search bar, a palette, a go-to-line prompt and a rename are four places
//! the reader types a line of text and one behaviour: the same keys move the
//! same caret and take out the same characters. This is that behaviour;
//! [`pm_ui::field`] is how it is drawn.

use std::ops::Range;

use winit::keyboard::{Key, ModifiersState, NamedKey};

/// One line of text being typed into.
#[derive(Clone, Debug, Default)]
pub struct Field {
    /// What is in the field.
    value: String,
    /// How many characters into it the caret sits.
    caret: usize,
    /// The other end of the selection, when a span is selected.
    anchor: Option<usize>,
}

impl Field {
    /// A field already holding `value`, with the caret at the end of it.
    pub fn filled(value: impl Into<String>) -> Self {
        let value = value.into();
        let caret = value.chars().count();
        Self {
            value,
            caret,
            anchor: None,
        }
    }

    /// What is in the field.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// How many characters into it the caret sits.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The character range that is selected, when a span is selected.
    pub fn selection(&self) -> Option<Range<usize>> {
        let (start, end) = self.span().filter(|(start, end)| start != end)?;
        Some(start..end)
    }

    /// Whether there is nothing in it.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Puts `value` in, leaving the caret at the end of it.
    pub fn set(&mut self, value: impl Into<String>) {
        *self = Self::filled(value);
    }

    /// Puts the caret `caret` characters in and drops the selection.
    pub fn place(&mut self, caret: usize) {
        self.caret = caret.min(self.value.chars().count());
        self.anchor = None;
    }

    /// Selects every character, leaving the caret at the end.
    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.caret = self.value.chars().count();
    }

    /// The selected span, when a span is selected.
    pub fn selected_text(&self) -> Option<&str> {
        let (start, end) = self.span().filter(|(start, end)| start != end)?;
        Some(&self.value[self.byte_of(start)..self.byte_of(end)])
    }

    /// Takes out the selected span and returns it, when a span is selected.
    pub fn cut_selection(&mut self) -> Option<String> {
        let text = self.selected_text()?.to_owned();
        self.take_selection();
        Some(text)
    }

    /// Replaces the selection, or inserts at the caret, with the first line of
    /// `text`.
    ///
    /// A field is one line, so a trailing `\r` or `\n` is trimmed and only
    /// the first line of a multi-line paste is kept.
    pub fn paste(&mut self, text: &str) {
        let line = text.lines().next().unwrap_or("");
        self.put(line);
    }

    /// Applies `key` to the line, saying whether it changed what is in it.
    ///
    /// A key the field has nothing to do with is left alone, so the caller
    /// can go on to treat it as the command it is — Enter and Escape belong
    /// to whatever the field is part of, not to the field.
    pub fn press(&mut self, key: &Key<&str>, modifiers: ModifiersState) -> Typed {
        let word = modifiers.control_key() || modifiers.alt_key();
        match *key {
            Key::Named(NamedKey::Backspace) if word => self.take_word_back(),
            Key::Named(NamedKey::Backspace) => self.take_back(),
            Key::Named(NamedKey::Delete) => self.take_forward(),
            Key::Named(NamedKey::ArrowLeft) if word => {
                self.collapse();
                self.caret = self.word_before();
            }
            Key::Named(NamedKey::ArrowRight) if word => {
                self.collapse();
                self.caret = self.word_after();
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.collapse();
                self.caret = self.caret.saturating_sub(1);
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.collapse();
                self.caret = (self.caret + 1).min(self.value.chars().count());
            }
            Key::Named(NamedKey::Home) => {
                self.collapse();
                self.caret = 0;
            }
            Key::Named(NamedKey::End) => {
                self.collapse();
                self.caret = self.value.chars().count();
            }
            Key::Named(NamedKey::Space) if !modifiers.control_key() => self.put(" "),
            Key::Character(text) if !modifiers.control_key() && !modifiers.super_key() => {
                self.put(text);
            }
            _ => return Typed::Ignored,
        }
        Typed::Taken
    }

    /// Puts `text` in where the caret is, replacing the selection when there
    /// is one.
    pub fn put(&mut self, text: &str) {
        self.take_selection();
        let at = self.byte_of(self.caret);
        self.value.insert_str(at, text);
        self.caret += text.chars().count();
    }

    /// Drops the selection, leaving the caret where it is.
    fn collapse(&mut self) {
        self.anchor = None;
    }

    /// The selected span as character offsets, when an anchor is set.
    fn span(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        Some(if anchor <= self.caret {
            (anchor, self.caret)
        } else {
            (self.caret, anchor)
        })
    }

    /// Takes out the selected span, leaving the caret at its start, saying
    /// whether there was one.
    fn take_selection(&mut self) -> bool {
        let Some((start, end)) = self.span().filter(|(start, end)| start != end) else {
            self.anchor = None;
            return false;
        };
        let (from, to) = (self.byte_of(start), self.byte_of(end));
        self.value.replace_range(from..to, "");
        self.caret = start;
        self.anchor = None;
        true
    }

    /// Takes out the character before the caret, or the selection when there
    /// is one.
    fn take_back(&mut self) {
        if self.take_selection() {
            return;
        }
        if self.caret == 0 {
            return;
        }
        let (from, to) = (self.byte_of(self.caret - 1), self.byte_of(self.caret));
        self.value.replace_range(from..to, "");
        self.caret -= 1;
    }

    /// Takes out the character after the caret, or the selection when there
    /// is one.
    fn take_forward(&mut self) {
        if self.take_selection() {
            return;
        }
        if self.caret >= self.value.chars().count() {
            return;
        }
        let (from, to) = (self.byte_of(self.caret), self.byte_of(self.caret + 1));
        self.value.replace_range(from..to, "");
    }

    /// Takes out the word before the caret, or the selection when there is
    /// one.
    fn take_word_back(&mut self) {
        if self.take_selection() {
            return;
        }
        let start = self.word_before();
        let (from, to) = (self.byte_of(start), self.byte_of(self.caret));
        self.value.replace_range(from..to, "");
        self.caret = start;
    }

    /// Where the word before the caret begins.
    fn word_before(&self) -> usize {
        let chars = self.value.chars().collect::<Vec<_>>();
        let mut at = self.caret;
        while at > 0 && chars[at - 1].is_whitespace() {
            at -= 1;
        }
        while at > 0 && !chars[at - 1].is_whitespace() {
            at -= 1;
        }
        at
    }

    /// Where the word after the caret ends.
    fn word_after(&self) -> usize {
        let chars = self.value.chars().collect::<Vec<_>>();
        let mut at = self.caret;
        while at < chars.len() && chars[at].is_whitespace() {
            at += 1;
        }
        while at < chars.len() && !chars[at].is_whitespace() {
            at += 1;
        }
        at
    }

    /// The byte offset `caret` characters in comes to.
    fn byte_of(&self, caret: usize) -> usize {
        self.value
            .char_indices()
            .nth(caret)
            .map_or(self.value.len(), |(at, _)| at)
    }
}

/// What a keypress aimed at a field came to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Typed {
    /// The field took the key.
    Taken,
    /// The key means nothing to a field, and is the caller's to deal with.
    Ignored,
}
