//! A single line of editable text and where the caret is in it.

/// One change to make to a [`Field`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Edit {
    /// Type a character at the caret.
    Insert(char),
    /// Delete the character before the caret.
    Backspace,
    /// Delete the character after the caret.
    Delete,
    /// Move the caret one character left.
    Left,
    /// Move the caret one character right.
    Right,
    /// Move the caret to the start.
    Home,
    /// Move the caret to the end.
    End,
}

/// A line of text with a caret between two of its characters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Field {
    /// The text.
    text: String,
    /// The caret, as a byte offset on a character boundary.
    caret: usize,
}

impl Field {
    /// A field holding `text`, with the caret at its end.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            caret: text.len(),
            text,
        }
    }

    /// The text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret, as a byte offset into [`Field::text`].
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Makes `edit`.
    pub fn apply(&mut self, edit: Edit) {
        match edit {
            Edit::Insert(character) if !character.is_control() => {
                self.text.insert(self.caret, character);
                self.caret += character.len_utf8();
            }
            Edit::Insert(_) => {}
            Edit::Backspace => {
                if let Some(start) = self.before() {
                    self.text.replace_range(start..self.caret, "");
                    self.caret = start;
                }
            }
            Edit::Delete => {
                if let Some(end) = self.after() {
                    self.text.replace_range(self.caret..end, "");
                }
            }
            Edit::Left => self.caret = self.before().unwrap_or(self.caret),
            Edit::Right => self.caret = self.after().unwrap_or(self.caret),
            Edit::Home => self.caret = 0,
            Edit::End => self.caret = self.text.len(),
        }
    }

    /// The offset of the character boundary before the caret.
    fn before(&self) -> Option<usize> {
        self.text[..self.caret]
            .char_indices()
            .next_back()
            .map(|(start, _)| start)
    }

    /// The offset of the character boundary after the caret.
    fn after(&self) -> Option<usize> {
        self.text[self.caret..]
            .chars()
            .next()
            .map(|character| self.caret + character.len_utf8())
    }
}
