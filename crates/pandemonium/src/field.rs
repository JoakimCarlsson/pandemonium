//! One line of text being typed into: what is in it, and where the caret is.
//!
//! A search bar, a palette, a go-to-line prompt and a rename are four places
//! the reader types a line of text and one behaviour: the same keys move the
//! same caret and take out the same characters. This is that behaviour;
//! [`pm_ui::field`] is how it is drawn.

use winit::keyboard::{Key, ModifiersState, NamedKey};

/// One line of text being typed into.
#[derive(Clone, Debug, Default)]
pub struct Field {
    /// What is in the field.
    value: String,
    /// How many characters into it the caret sits.
    caret: usize,
}

impl Field {
    /// A field already holding `value`, with the caret at the end of it.
    pub fn filled(value: impl Into<String>) -> Self {
        let value = value.into();
        let caret = value.chars().count();
        Self { value, caret }
    }

    /// What is in the field.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// How many characters into it the caret sits.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Whether there is nothing in it.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Puts `value` in, leaving the caret at the end of it.
    pub fn set(&mut self, value: impl Into<String>) {
        *self = Self::filled(value);
    }

    /// Puts the caret `caret` characters in.
    pub fn place(&mut self, caret: usize) {
        self.caret = caret.min(self.value.chars().count());
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
            Key::Named(NamedKey::ArrowLeft) if word => self.caret = self.word_before(),
            Key::Named(NamedKey::ArrowRight) if word => self.caret = self.word_after(),
            Key::Named(NamedKey::ArrowLeft) => self.caret = self.caret.saturating_sub(1),
            Key::Named(NamedKey::ArrowRight) => {
                self.caret = (self.caret + 1).min(self.value.chars().count());
            }
            Key::Named(NamedKey::Home) => self.caret = 0,
            Key::Named(NamedKey::End) => self.caret = self.value.chars().count(),
            Key::Named(NamedKey::Space) if !modifiers.control_key() => self.put(" "),
            Key::Character(text) if !modifiers.control_key() && !modifiers.super_key() => {
                self.put(text);
            }
            _ => return Typed::Ignored,
        }
        Typed::Taken
    }

    /// Puts `text` in where the caret is.
    pub fn put(&mut self, text: &str) {
        let at = self.byte_of(self.caret);
        self.value.insert_str(at, text);
        self.caret += text.chars().count();
    }

    /// Takes out the character before the caret.
    fn take_back(&mut self) {
        if self.caret == 0 {
            return;
        }
        let (from, to) = (self.byte_of(self.caret - 1), self.byte_of(self.caret));
        self.value.replace_range(from..to, "");
        self.caret -= 1;
    }

    /// Takes out the character after the caret.
    fn take_forward(&mut self) {
        if self.caret >= self.value.chars().count() {
            return;
        }
        let (from, to) = (self.byte_of(self.caret), self.byte_of(self.caret + 1));
        self.value.replace_range(from..to, "");
    }

    /// Takes out the word before the caret.
    fn take_word_back(&mut self) {
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
