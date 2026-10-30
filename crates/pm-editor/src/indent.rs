//! Indentation as the language says: a new line one level deeper inside a block opened on the line, the closing of that block on a line of its own, and a closing bracket typed on an empty line going back a level.

use pm_text::Edit;

use crate::document::Document;

impl Document {
    /// The edits that break the line at the caret: the new line indented as the language says, and, when the caret sits between a block's opening and its closing, the closing put on a line of its own at the level of the line broken, the caret left on the line between.
    pub fn newline_edits(&self) -> Vec<Edit> {
        self.newline_edits_with(true, false)
    }

    /// The edits that break the line at the caret: indented as the language says when `indent`, and starting with the line comment too when `comments` and the line broken is a comment that goes on after the caret.
    pub fn newline_edits_with(&self, indent: bool, comments: bool) -> Vec<Edit> {
        let caret = self.field.caret();
        let text = self.field.text();
        let base = self.indentation_at(caret);
        if comments && let Some(prefix) = self.line_comment_at(caret).as_deref() {
            let start = self.line(self.line_of(caret)).start;
            let before = &text[start..caret];
            let trimmed = before.trim_start();
            if trimmed.starts_with(prefix) && trimmed.len() > prefix.len() {
                let marker: String = trimmed
                    .chars()
                    .take_while(|character| prefix.contains(*character))
                    .collect();
                return vec![Edit::Paste(format!("\n{base}{marker} "))];
            }
        }
        if !indent {
            return vec![Edit::Insert('\n')];
        }
        self.indented_newline()
    }

    /// The edits that break the line at the caret, indented as the language says.
    fn indented_newline(&self) -> Vec<Edit> {
        let caret = self.field.caret();
        let text = self.field.text();
        let base = self.indentation_at(caret);
        let unit = self.editing.unit(1);
        let indent = self
            .syntax
            .as_ref()
            .map(|syntax| syntax.indent_at(text, self.line_index(), caret))
            .unwrap_or_default();
        let before = text[self.line(self.line_of(caret)).start..caret].trim_end();
        let deeper = indent.deeper
            || before.ends_with(['{', '(', '[', ':'])
            || self.editing.indent_rules.increases(before);
        let closing = deeper
            && text[caret..]
                .trim_start_matches([' ', '\t'])
                .starts_with(['}', ')', ']']);
        let inner = if deeper {
            format!("{base}{unit}")
        } else {
            base.clone()
        };
        let start = self.field.selection().map_or(caret, |range| range.start);
        let mut edits = vec![Edit::Paste(format!("\n{inner}"))];
        if closing {
            edits.push(Edit::Paste(format!("\n{base}")));
            edits.push(Edit::Place(start + 1 + inner.len(), false));
        }
        edits
    }

    /// The edits that take the line the caret is on back as the language's line patterns say, once `character` has just been typed and made the line start matching; none when no pattern does.
    fn pattern_outdent_edits(&self, character: char) -> Vec<Edit> {
        let rules = &self.editing.indent_rules;
        if rules.is_empty() {
            return Vec::new();
        }
        let text = self.field.text();
        let caret = self.field.caret();
        let row = self.line_of(caret);
        let line = self.line(row);
        let upto = &text[line.start..caret];
        let before = &text[line.start..caret.saturating_sub(character.len_utf8())];
        let leading_end = upto.len() - upto.trim_start_matches([' ', '\t']).len();
        let leading = &upto[..leading_end];
        let matched = |held: &str| rules.decreases(held) || rules.aligning(held).is_some();
        if !matched(upto.trim_start()) || matched(before.trim_start()) {
            return Vec::new();
        }
        let unit = self.editing.unit(1);
        let indentation = match rules.aligning(upto.trim_start()) {
            Some(rule) if !rule.valid_after.is_empty() => self
                .syntax
                .as_ref()
                .map(|syntax| syntax.construct_starts(text, line.start, &rule.valid_after))
                .unwrap_or_default()
                .into_iter()
                .rev()
                .map(|(start_row, _)| {
                    let held = self.line(start_row);
                    let words = &text[held.clone()];
                    words[..words.len() - words.trim_start_matches([' ', '\t']).len()].to_owned()
                })
                .find(|candidate| candidate.len() <= leading.len()),
            _ => leading.strip_suffix(&unit).map(str::to_owned),
        };
        let Some(indentation) = indentation.filter(|held| held != leading) else {
            return Vec::new();
        };
        let after = caret - leading.len() + indentation.len();
        vec![Edit::Replace(
            line.start..line.start + leading.len(),
            indentation,
            (after, after),
        )]
    }

    /// The edits that take a closing bracket `character`, just typed where its line held nothing else, back one level of indentation, or the line back as the language's line patterns say; none when it is neither.
    pub fn outdent_edits(&self, character: char) -> Vec<Edit> {
        let bracket = self.bracket_outdent_edits(character);
        if bracket.is_empty() {
            self.pattern_outdent_edits(character)
        } else {
            bracket
        }
    }

    /// The edits that take a closing bracket `character`, just typed where its line held nothing else, back one level of indentation; none when it is not one or does not close a block.
    fn bracket_outdent_edits(&self, character: char) -> Vec<Edit> {
        if !matches!(character, '}' | ')' | ']') {
            return Vec::new();
        }
        let caret = self.field.caret();
        let at = caret.saturating_sub(character.len_utf8());
        let line = self.line(self.line_of(at));
        let leading = &self.field.text()[line.start..at];
        let closes = self
            .syntax
            .as_ref()
            .is_some_and(|syntax| syntax.closes_at(self.field.text(), at));
        if !closes || leading.is_empty() || !leading.chars().all(|c| c == ' ' || c == '\t') {
            return Vec::new();
        }
        let unit = self.editing.unit(1);
        let Some(shorter) = leading.strip_suffix(&unit) else {
            return Vec::new();
        };
        let start = line.start + shorter.len();
        vec![
            Edit::Place(start, false),
            Edit::Place(at, true),
            Edit::Backspace,
            Edit::Place(start + character.len_utf8(), false),
        ]
    }
}
