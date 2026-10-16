//! What the language server says about the caret's surroundings as the reader moves and types: where the thing at the caret is used, and the signature of the call the caret is in.

use pm_protocol::{Signature, Span};

use crate::document::Document;

impl Document {
    /// Takes in `spans`, where the thing at the caret is used, as the language server found them in the text at `version`.
    pub fn take_highlights(&mut self, version: i32, spans: Vec<Span>) {
        let ranges = spans.into_iter().map(|span| self.range_of(span)).collect();
        self.occurrences = (version, ranges);
    }

    /// The stretches where the thing at the caret is used, when they were found in the text as it is now.
    pub fn occurrences(&self) -> &[std::ops::Range<usize>] {
        if self.occurrences.0 == self.version {
            &self.occurrences.1
        } else {
            &[]
        }
    }

    /// Takes in `signature`, the signature of the call the caret is in as the language server said for the text at `version`, or that it is in none.
    pub fn take_signature(&mut self, version: i32, signature: Option<Signature>) {
        if version == self.version {
            self.signature = signature;
        }
    }
}
