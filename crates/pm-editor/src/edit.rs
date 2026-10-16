//! Changing a document's text: the syntax tree reparsed, the version the language server was told of moved on, and the completions open at the caret kept in step.

use pm_buffer::Lines;
use pm_protocol::TextChange;
use pm_text::Edit;

use crate::document::{Completions, Document};

/// What an edit that changed the text calls for.
#[derive(Clone, Debug, PartialEq)]
pub struct Edited {
    /// What the language server is told, when it reads the file: the version the text reached and the stretch replaced.
    pub change: Option<(i32, TextChange)>,
    /// Completions to ask the language server for, as the character typed that asked when one did; the list is open and waiting for them already.
    pub ask: Option<Option<String>>,
    /// Whether to ask the language server what the call the caret is in looks like: a character that opens or separates arguments was typed, or the signature showing follows the text.
    pub signature: bool,
}

impl Document {
    /// Makes `edit`, and says what it calls for when it changed the text; an edit that only moved the caret narrows the completions open and calls for nothing.
    pub fn edit(&mut self, edit: Edit) -> Option<Edited> {
        let before = self.field.text().to_owned();
        let before_lines: Lines = self.line_index().clone();
        let caret_line = self.line_of(self.field.caret());
        self.field.apply(edit);
        let Some(change) = self.refresh(&before, &before_lines) else {
            self.carry_folds(None);
            self.refilter(false);
            if self.line_of(self.field.caret()) != caret_line {
                self.signature = None;
            }
            return None;
        };
        self.hover = None;
        self.grown.clear();
        self.carry_folds(Some(&change));
        let typed: Option<char> = (change.range.is_empty() && change.text.chars().count() == 1)
            .then(|| change.text.chars().next())
            .flatten();
        let told = self.synced.then(|| {
            self.version += 1;
            (
                self.version,
                Document::text_change(&change, &before, &before_lines),
            )
        });
        let trigger =
            typed.filter(|character| self.triggers.iter().any(|held| held.ends_with(*character)));
        let opened = (change.range.is_empty() && change.text.chars().count() <= 2)
            .then(|| change.text.chars().next())
            .flatten();
        let signature = self.synced
            && (self.signature.is_some()
                || opened.is_some_and(|character| {
                    self.signature_triggers
                        .iter()
                        .any(|held| held.ends_with(character))
                }));
        let reopen = self.refilter(trigger.is_some());
        let ask = match (trigger, reopen) {
            (Some(character), _) => Some(Some(character.to_string())),
            (None, true) => Some(None),
            (None, false) => None,
        }
        .filter(|_| self.synced);
        if ask.is_some() {
            let at = self.completion.as_ref().map_or((0.0, 0.0), |held| held.at);
            self.open_completion(at);
        }
        Some(Edited {
            change: told,
            ask,
            signature,
        })
    }

    /// Opens the list of completions at the caret, waiting for the language server, hanging at `at`; returns the version and place they are asked for at.
    pub fn open_completion(&mut self, at: (f32, f32)) -> (i32, pm_protocol::Position) {
        let caret = self.field.caret();
        self.completion = Some(Completions {
            start: self.word_start(caret),
            items: Vec::new(),
            shown: Vec::new(),
            picked: 0,
            at,
            asked: self.version,
            loading: true,
        });
        (self.version, self.position_of(caret))
    }
}
