//! The marker beside the line of the caret that says the language server offers changes there.

use crate::document::Document;

/// What the language server said it offers at the caret.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Bulb {
    /// The version of the text it was asked about.
    pub(crate) version: i32,
    /// The line the caret was on.
    pub(crate) line: usize,
    /// How many changes it offered that can be made.
    pub(crate) offered: usize,
}

impl Document {
    /// Takes in how many changes the language server offered at the caret on `line` of the text at `version`.
    pub fn take_bulb(&mut self, version: i32, line: usize, offered: usize) {
        self.bulb = Some(Bulb {
            version,
            line,
            offered,
        });
    }

    /// The line the marker stands beside: the line of the caret, when changes were offered there for the text as it is now; and when `only_problems`, only while that line has a problem marked.
    pub fn bulb_line(&self, only_problems: bool) -> Option<usize> {
        let bulb = self.bulb.filter(|bulb| bulb.offered > 0)?;
        let line = self.line_of(self.field.caret());
        (bulb.version == self.version
            && bulb.line == line
            && (!only_problems || !self.diagnostics_on(line).is_empty()))
        .then_some(line)
    }
}
