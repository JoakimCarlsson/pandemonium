//! The text of a file open in the window: edited in place, line by line, and told apart from what was last saved.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use pm_text::Field;

/// A file's text, the caret and selection in it, and whether it differs from what the folder holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    /// The text, with the caret, the selection and what undo remembers.
    pub field: Field,
    /// Where each line starts, as a byte offset into the text.
    starts: Vec<usize>,
    /// A fingerprint of the text as it is now.
    current: u64,
    /// A fingerprint of the text as the folder holds it, or nothing once saving it failed.
    saved: Option<u64>,
    /// Whether the file ended its lines with a carriage return as well, which saving puts back.
    crlf: bool,
    /// Whether the file was longer than could be read, so that saving would cut it.
    pub truncated: bool,
}

/// A fingerprint of `text`.
fn fingerprint(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Where each line of `text` starts.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(at, _)| at + 1))
        .collect()
}

impl Document {
    /// The document of a file that held `text`, which cannot be changed when `read_only`; a file read only in part is always read only.
    pub fn new(text: &str, read_only: bool, truncated: bool) -> Self {
        let crlf = text.contains("\r\n");
        let text = if crlf {
            text.replace("\r\n", "\n")
        } else {
            text.to_owned()
        };
        let starts = line_starts(&text);
        let current = fingerprint(&text);
        Self {
            field: Field::new(text)
                .multiline(true)
                .read_only(read_only || truncated),
            starts,
            current,
            saved: Some(current),
            crlf,
            truncated,
        }
    }

    /// Notes what the text has become after an edit, and says whether it changed.
    pub fn refresh(&mut self) -> bool {
        let current = fingerprint(self.field.text());
        if current == self.current {
            return false;
        }
        self.current = current;
        self.starts = line_starts(self.field.text());
        true
    }

    /// Whether the text differs from what the folder holds.
    pub fn is_dirty(&self) -> bool {
        self.saved != Some(self.current)
    }

    /// Notes that the folder now holds the text as it is.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.current);
    }

    /// Notes that the folder may not hold the text, because writing it failed.
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
    }

    /// The text as it is written to the folder, with the file's own line endings.
    pub fn to_save(&self) -> String {
        if self.crlf {
            self.field.text().replace('\n', "\r\n")
        } else {
            self.field.text().to_owned()
        }
    }

    /// How many lines there are.
    pub fn lines(&self) -> usize {
        self.starts.len()
    }

    /// Where line `index` is in the text, without its line break.
    pub fn line(&self, index: usize) -> Range<usize> {
        let start = self.starts[index];
        let end = self
            .starts
            .get(index + 1)
            .map_or(self.field.text().len(), |next| next - 1);
        start..end
    }

    /// The line the byte `offset` of the text is on.
    pub fn line_of(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }
}
