//! The stretches of a file that hold private values, by the line each starts on.

use std::fmt;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use crate::document::Document;

/// The stretches of a text that hold private values.
type Stretches = Arc<Vec<Range<usize>>>;

/// The stretches found last and the parse they came from, shared by the copies of a document so a frame does not look for them again.
#[derive(Clone, Default)]
pub(crate) struct RedactCache(Arc<Mutex<(u64, Stretches)>>);

impl fmt::Debug for RedactCache {
    /// Says nothing of what is held.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RedactCache")
    }
}

impl PartialEq for RedactCache {
    /// What is held is worked out from the rest of the document, so it never tells two documents apart.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Document {
    /// The bytes of the file that its language marks as private values, in order, looked for again only when the text was parsed again.
    pub fn redactions(&self) -> Arc<Vec<Range<usize>>> {
        let Some(syntax) = &self.syntax else {
            return Stretches::default();
        };
        let Ok(mut held) = self.redacts.0.lock() else {
            return Stretches::default();
        };
        if held.0 != syntax.revision() {
            *held = (
                syntax.revision(),
                Arc::new(syntax.redactions(self.field.text())),
            );
        }
        held.1.clone()
    }
}
