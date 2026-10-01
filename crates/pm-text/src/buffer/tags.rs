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

    /// The name the cursor is in, or at either end of, when what is before
    /// it in the line is a `<` or a `</`.
    ///
    /// This is read from the text and not the tree, so that a name that has
    /// been taken away entirely, which no grammar parses as a tag, is still
    /// the name of a tag.
    fn name_at_cursor(&self) -> Option<Range<Position>> {
        let head = self.selection().head;
        let chars = self.line_chars(head.line).collect::<Vec<_>>();
        let is_name = |ch: char| ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':');
        let mut start = head.column.min(chars.len());
        while start > 0 && is_name(chars[start - 1]) {
            start -= 1;
        }
        let mut end = head.column.min(chars.len());
        while end < chars.len() && is_name(chars[end]) {
            end += 1;
        }
        let opens = start > 0
            && (chars[start - 1] == '<'
                || (start > 1 && chars[start - 1] == '/' && chars[start - 2] == '<'));
        opens.then(|| Position::new(head.line, start)..Position::new(head.line, end))
    }

    /// Whether the cursor is in the name of a tag whose partner is named the
    /// same, which is when an edit to one is to be carried to the other.
    ///
    /// The partner is written down on the way, so that it can still be found
    /// after the edit has left no tag the grammar can pair it with.
    pub fn in_linked_tag(&mut self) -> bool {
        let Some(here) = self
            .name_at_cursor()
            .filter(|_| self.selection().is_empty())
        else {
            self.twin = None;
            return false;
        };
        let name = self.text_in(here);
        if let Some((here, there)) = self.tag_names_at_cursor()
            && self.text_in(here) == name
            && self.text_in(there.clone()) == name
        {
            self.twin = Some(there);
            return true;
        }
        let kept = self
            .twin
            .clone()
            .is_some_and(|twin| self.text_in(twin) == name);
        if !kept {
            self.twin = None;
        }
        kept
    }

    /// Gives the tag the cursor is not in the name the cursor's tag has now.
    ///
    /// The cursor stays where it was in the text it was typing into, even
    /// when the other tag comes before it and changes length.
    pub fn mirror_tag_name(&mut self) {
        let Some(here) = self.name_at_cursor() else {
            self.twin = None;
            return;
        };
        let name = self.text_in(here.clone());
        let there = match self.tag_names_at_cursor() {
            Some((found, there)) if self.text_in(found.clone()) == name => Some(there),
            _ => self.twin.clone(),
        };
        let Some(there) = there else {
            return;
        };
        if self.text_in(there.clone()) == name {
            self.twin = Some(there);
            return;
        }
        let head = self.char_of(self.selection().head);
        let before = there.start < here.start;
        let (start, end) = (self.char_of(there.start), self.char_of(there.end));
        let twin = there.start..there.start.after(&name);
        self.replace(there, &name);
        let head = match before {
            true => (head + name.chars().count()).saturating_sub(end - start),
            false => head,
        };
        let head = self.position_of(head);
        self.set_selection(Selection::at(head));
        self.twin = Some(twin);
    }
}
