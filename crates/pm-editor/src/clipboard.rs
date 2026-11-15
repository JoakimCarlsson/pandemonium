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

    /// The selected text with the indentation its lines share taken off, the line it starts on counted with the spaces and tabs before the selection; nothing when nothing is selected.
    pub fn copied_without_indentation(&self) -> Option<String> {
        let range = self.field.selection()?;
        let start = self.line(self.line_of(range.start)).start;
        let document = self.field.text();
        Some(without_shared_indentation(
            &document[range.clone()],
            &document[start..range.start],
        ))
    }
}

/// Whether `character` indents a line.
fn indents(character: char) -> bool {
    character == ' ' || character == '\t'
}

/// The spaces and tabs `line` begins with.
fn leading(line: &str) -> &str {
    &line[..line.len() - line.trim_start_matches(indents).len()]
}

/// How many bytes `first` and `second` begin with alike.
fn shared_prefix(first: &str, second: &str) -> usize {
    first
        .bytes()
        .zip(second.bytes())
        .take_while(|(left, right)| left == right)
        .count()
}

/// `text` with the indentation its lines that hold more than spaces share taken off, where `before` is what stands on its first line before it: the first line counts with `before` when that is only spaces and tabs, and is left as it is otherwise.
pub(crate) fn without_shared_indentation(text: &str, before: &str) -> String {
    let indented_start = before.chars().all(indents);
    let mut lines = text.split('\n');
    let first = lines.next().unwrap_or_default();
    let first_whole = format!("{before}{first}");
    let rest: Vec<&str> = lines.collect();
    let counted = indented_start
        .then_some(first_whole.as_str())
        .into_iter()
        .chain(rest.iter().copied())
        .filter(|line| !line.trim().is_empty());
    let mut shared: Option<&str> = None;
    for line in counted {
        let own = leading(line);
        shared = Some(match shared {
            Some(held) => &held[..shared_prefix(held, own)],
            None => own,
        });
    }
    let shared = shared.unwrap_or_default();
    let mut out = String::with_capacity(text.len());
    if indented_start {
        let cut = shared
            .len()
            .saturating_sub(before.len())
            .min(leading(first).len());
        out.push_str(&first[cut..]);
    } else {
        out.push_str(first);
    }
    for line in rest {
        out.push('\n');
        out.push_str(&line[shared_prefix(line, shared).min(leading(line).len())..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines indented alike lose the indentation they share and keep the rest.
    #[test]
    fn shared_indentation_comes_off() {
        assert_eq!(
            without_shared_indentation("    a\n        b\n\n    c", ""),
            "a\n    b\n\nc"
        );
    }

    /// A selection started inside the indentation counts the spaces before it.
    #[test]
    fn indentation_before_the_selection_counts() {
        assert_eq!(without_shared_indentation("  a\n      b", "  "), "a\n  b");
    }

    /// A selection started after text leaves its first line alone.
    #[test]
    fn first_line_after_text_stays() {
        assert_eq!(
            without_shared_indentation("x = 1\n    y\n    z", "let "),
            "x = 1\ny\nz"
        );
    }

    /// The document copies its selection without the indentation it shares.
    #[test]
    fn document_copies_without_indentation() {
        let mut document = Document::new("fn f() {\n    a();\n        b();\n}\n", false, false);
        let start = document.line(1).start;
        let end = document.line(2).end;
        document.field.apply(Edit::Select(start, end));
        assert_eq!(
            document.copied_without_indentation().as_deref(),
            Some("a();\n    b();")
        );
    }
}
