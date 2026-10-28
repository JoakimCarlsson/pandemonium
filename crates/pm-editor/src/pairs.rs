//! Brackets and quotes typed in pairs: the closing one written with the opening one, typed over when it is next, wrapped around a selection, and deleted with the opening one; and backspace in indentation taking a whole level.

use pm_text::Edit;

use crate::document::Document;

/// Whether a pair may be opened before `next`, the character after the caret: at the end of a line or of the text, before space, or before something that closes or separates.
fn opens_before(next: Option<char>) -> bool {
    next.is_none_or(|next| {
        next.is_whitespace() || matches!(next, ')' | ']' | '}' | ',' | ';' | ':' | '.' | '>')
    })
}

impl Document {
    /// The characters before and after the caret.
    fn around_caret(&self) -> (Option<char>, Option<char>) {
        let text = self.field.text();
        let caret = self.field.caret();
        (
            text[..caret].chars().next_back(),
            text[caret..].chars().next(),
        )
    }

    /// Whether the caret is inside a string or a comment, where what is typed is text rather than code.
    fn caret_in_text(&self) -> bool {
        self.syntax
            .as_ref()
            .is_some_and(|syntax| syntax.in_text(self.field.caret()))
    }

    /// Whether the language does not write the pair `open` opens at the caret, because the scope the caret is in is one it is `not_in`.
    fn pair_excluded(&self, open: char) -> bool {
        let Some(scope) = self.scope_name() else {
            return false;
        };
        self.editing
            .not_in
            .iter()
            .any(|(held, names)| *held == open && names.contains(&scope))
    }

    /// The edit that types `character` as the language pairs it, or nothing when it is typed as it is: a selection is wrapped in the pair it opens, an opening character is written with its closing one, and a closing character is typed over when it is already next.
    pub fn typed_edit(&self, character: char) -> Option<Edit> {
        self.typed_edit_with(character, true, true)
    }

    /// The edit that types `character` as the language pairs it, closing what it opens only when `close` and going around a selection only when `surround`, or nothing when it is typed as it is.
    pub fn typed_edit_with(&self, character: char, close: bool, surround: bool) -> Option<Edit> {
        let caret = self.field.caret();
        if character == '>'
            && close
            && self.field.selection().is_none()
            && let Some(edit) = self.closing_tag_edit()
        {
            return Some(edit);
        }
        let pair = self
            .editing
            .pairs
            .iter()
            .find(|(open, _)| *open == character)
            .copied();
        if let (Some((open, close)), Some(range)) =
            (pair.filter(|_| surround), self.field.selection())
        {
            let selected = &self.field.text()[range.clone()];
            let wrapped = format!("{open}{selected}{close}");
            let start = range.start + open.len_utf8();
            return Some(Edit::Replace(
                range.clone(),
                wrapped,
                (start, start + selected.len()),
            ));
        }
        let (before, next) = self.around_caret();
        let closes = self
            .editing
            .pairs
            .iter()
            .any(|(_, close)| *close == character);
        if closes && next == Some(character) {
            return Some(Edit::Place(caret + character.len_utf8(), false));
        }
        let (open, close) = pair.filter(|_| close)?;
        let stated = self.editing.not_in.iter().any(|(held, _)| *held == open);
        let barred = if stated {
            self.pair_excluded(open)
        } else {
            self.caret_in_text()
        };
        if barred || !opens_before(next) {
            return None;
        }
        let quote = open == close;
        if quote && before.is_some_and(|before| before.is_alphanumeric() || before == open) {
            return None;
        }
        let inside = caret + open.len_utf8();
        Some(Edit::Replace(
            caret..caret,
            format!("{open}{close}"),
            (inside, inside),
        ))
    }

    /// The edit backspace makes at the caret when it does more than delete one character: an empty pair is deleted whole, and in the indentation at the start of a line of spaces it goes back to the level before; nothing otherwise.
    pub fn backspace_edit(&self) -> Option<Edit> {
        if self.field.selection().is_some() {
            return None;
        }
        let caret = self.field.caret();
        let (before, next) = self.around_caret();
        if let (Some(before), Some(next)) = (before, next)
            && self.editing.pairs.contains(&(before, next))
        {
            let start = caret - before.len_utf8();
            return Some(Edit::Replace(
                start..caret + next.len_utf8(),
                String::new(),
                (start, start),
            ));
        }
        let text = self.field.text();
        let line_start = text[..caret].rfind('\n').map_or(0, |at| at + 1);
        let leading = &text[line_start..caret];
        if self.editing.tabs
            || leading.len() < 2
            || !leading.chars().all(|character| character == ' ')
        {
            return None;
        }
        let unit = self.editing.indent.max(1);
        let step = match leading.len() % unit {
            0 => unit,
            rest => rest,
        };
        let start = caret - step;
        Some(Edit::Replace(start..caret, String::new(), (start, start)))
    }

    /// The edit that grows the selection to the smallest piece of syntax that holds it and more, remembering what it was so it can shrink back.
    pub fn grow_selection_edit(&mut self) -> Option<Edit> {
        let caret = self.field.caret();
        let range = self.field.selection().unwrap_or(caret..caret);
        let larger = self.syntax.as_ref()?.larger_node(range.clone())?;
        if self.grown.last().is_none_or(|(_, to)| *to != range) {
            self.grown.clear();
        }
        self.grown.push((range, larger.clone()));
        Some(Edit::Select(larger.start, larger.end))
    }

    /// The edit that selects the `object` around the caret, or what is inside it when `inside`, as the language's text objects say; none when the language has none there.
    pub fn select_object_edit(
        &self,
        object: pm_treesitter::TextObject,
        inside: bool,
    ) -> Option<Edit> {
        let range = self.syntax.as_ref()?.text_object(
            self.field.text(),
            object,
            inside,
            self.field.caret(),
        )?;
        Some(Edit::Select(range.start, range.end))
    }

    /// The edit that shrinks the selection back to what it was before it last grew, when it is still what it grew to.
    pub fn shrink_selection_edit(&mut self) -> Option<Edit> {
        let caret = self.field.caret();
        let range = self.field.selection().unwrap_or(caret..caret);
        let (before, to) = self.grown.pop()?;
        if to != range {
            self.grown.clear();
            return None;
        }
        Some(Edit::Select(before.start, before.end))
    }
}
