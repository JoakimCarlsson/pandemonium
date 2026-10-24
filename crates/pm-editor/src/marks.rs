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

    /// Takes in `spans`, the stretches edited together with the one at the caret as the language server said for the text at `version`; they are kept while the text is as it was then and the caret is in one of them, and there are at least two.
    pub fn take_linked(&mut self, version: i32, spans: Vec<Span>) {
        let caret = self.field.caret();
        let mut ranges: Vec<std::ops::Range<usize>> =
            spans.into_iter().map(|span| self.range_of(span)).collect();
        ranges.sort_by_key(|range| range.start);
        let holds_caret = ranges
            .iter()
            .any(|range| range.start <= caret && caret <= range.end);
        let apart = ranges.windows(2).all(|pair| pair[0].end <= pair[1].start);
        self.linked =
            (version == self.version && ranges.len() > 1 && holds_caret && apart).then_some(ranges);
    }

    /// Takes in `ranges`, the blocks of lines the language server said can be folded in the text at `version`.
    pub fn take_folds(&mut self, version: i32, ranges: Vec<pm_protocol::FoldRange>) {
        self.server_folds = (version, ranges);
    }

    /// The version of the text the blocks the language server said can be folded are about, when it said.
    pub fn server_folds_version(&self) -> Option<i32> {
        (self.server_folds.0 != 0 || !self.server_folds.1.is_empty()).then_some(self.server_folds.0)
    }

    /// Whether the blocks the language server said can be folded are about the text as it is now.
    pub fn has_server_folds(&self) -> bool {
        self.server_folds.0 == self.version && !self.server_folds.1.is_empty()
    }
}
