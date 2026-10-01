//! Opening and closing tags that are renamed together.

use std::ops::Range;

use crate::buffer::Buffer;
use crate::cursor::{Position, Selection};

impl Buffer {
    /// The names of the two tags of the element the cursor is in the name of,
    /// the one it is in first, when the cursor is in the name of a tag.
    fn tag_names_at_cursor(&self) -> Option<(Range<Position>, Range<Position>)> {
        let head = self.char_of(self.selection().head);
        let byte = self.text.char_to_byte(head);
        let (here, there) = self.syntax.as_ref()?.tag_names(byte)?;
        let span = |bytes: Range<usize>| {
            self.position_of(self.text.byte_to_char(bytes.start))
                ..self.position_of(self.text.byte_to_char(bytes.end))
        };
        Some((span(here), span(there)))
    }

    /// Whether the cursor is in the name of a tag whose partner is named the
    /// same, which is when an edit to one is to be carried to the other.
    pub fn in_linked_tag(&self) -> bool {
        self.selection().is_empty()
            && self
                .tag_names_at_cursor()
                .is_some_and(|(here, there)| self.text_in(here) == self.text_in(there))
    }

    /// Gives the tag the cursor is not in the name the cursor's tag has now.
    ///
    /// The cursor stays where it was in the text it was typing into, even
    /// when the other tag comes before it and changes length.
    pub fn mirror_tag_name(&mut self) {
        let Some((here, there)) = self.tag_names_at_cursor() else {
            return;
        };
        let name = self.text_in(here.clone());
        if name == self.text_in(there.clone()) {
            return;
        }
        let head = self.char_of(self.selection().head);
        let before = there.start < here.start;
        let (start, end) = (self.char_of(there.start), self.char_of(there.end));
        self.replace(there, &name);
        let head = match before {
            true => (head + name.chars().count()).saturating_sub(end - start),
            false => head,
        };
        let head = self.position_of(head);
        self.set_selection(Selection::at(head));
    }
}
