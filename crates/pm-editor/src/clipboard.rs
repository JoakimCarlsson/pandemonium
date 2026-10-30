//! What copying, cutting and pasting do in a file that a plain field does not: the lines copied when nothing is selected, pasted above the line of the caret, and lines of code pasted at the indentation of the place they go to.

use pm_text::Edit;

use crate::document::Document;

impl Document {
    /// The lines every caret is on, each with its line break, when nothing is selected; nothing when something is.
    pub fn copied_lines(&self) -> Option<String> {
        let selections = self.field.selections();
        if selections.iter().any(|(anchor, caret)| anchor != caret) {
            return None;
        }
        let text = self.field.text();
        let mut starts: Vec<usize> = Vec::new();
        let mut copied = String::new();
        for (_, caret) in selections {
            let lines = self.lines_around(caret..caret);
            if starts.contains(&lines.start) {
                continue;
            }
            starts.push(lines.start);
            copied.push_str(&text[lines]);
            if !copied.ends_with('\n') {
                copied.push('\n');
            }
        }
        (!copied.is_empty()).then_some(copied)
    }

    /// The spaces and tabs the line the selection starts on begins with, which the lines copied from it are indented by.
    pub fn copied_indentation(&self) -> String {
        let start = self
            .field
            .selection()
            .map_or(self.field.caret(), |range| range.start);
        self.indentation_at(start)
    }

    /// The edit that pastes `text`, which was copied as whole lines, above the line of the caret, the caret staying in its line; nothing while something is selected or there is more than one caret.
    pub fn paste_lines_edit(&self, text: &str) -> Option<Edit> {
        if self.field.selection().is_some() || self.field.has_others() {
            return None;
        }
        let caret = self.field.caret();
        let start = self.lines_around(caret..caret).start;
        let at = caret + text.len();
        Some(Edit::Replace(start..start, text.to_owned(), (at, at)))
    }

    /// The edit that pastes `text`, copied from lines indented by `source`, so that its lines keep their indentation relative to the line it goes into; nothing when it is one line, when text comes before the place on its line, or when it needs no change.
    pub fn paste_reindented_edit(&self, text: &str, source: &str) -> Option<Edit> {
        if self.field.has_others() || !text.contains('\n') {
            return None;
        }
        let document = self.field.text();
        let at = self
            .field
            .selection()
            .map_or(self.field.caret(), |range| range.start);
        let line = self.line(self.line_of(at));
        let before = &document[line.start..at];
        if !before
            .chars()
            .all(|character| character == ' ' || character == '\t')
        {
            return None;
        }
        let target: String = if before.is_empty() {
            document[..line.start]
                .lines()
                .rev()
                .find(|held| !held.trim().is_empty())
                .map(|held| {
                    held.chars()
                        .take_while(|character| *character == ' ' || *character == '\t')
                        .collect()
                })
                .unwrap_or_default()
        } else {
            before.to_owned()
        };
        let mut lines = text.split('\n');
        let first = lines.next()?;
        let first = first.strip_prefix(source).unwrap_or(first);
        let mut pasted = String::with_capacity(text.len() + target.len());
        if before.is_empty() {
            pasted.push_str(&target);
        }
        pasted.push_str(first);
        for line in lines {
            pasted.push('\n');
            if line.trim().is_empty() {
                continue;
            }
            let shared = line
                .chars()
                .zip(source.chars())
                .take_while(|(held, wanted)| held == wanted)
                .count();
            pasted.push_str(&target);
            pasted.push_str(&line[shared..]);
        }
        (pasted != text).then_some(Edit::Paste(pasted))
    }
}
