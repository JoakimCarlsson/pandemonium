//! The places of a file that can be run, by the line each starts on.

use std::fmt;
use std::sync::{Arc, Mutex};

use pm_treesitter::Runnable;

use crate::document::Document;

/// The most bytes of a text whose runnable places are looked for, beyond which none are.
const MOST_BYTES: usize = 2 * 1024 * 1024;

/// The runnable places of a text, each with the line it starts on, in order.
type Places = Arc<Vec<(usize, Runnable)>>;

/// The places found last and the parse they came from, shared by the copies of a document so a frame does not look for them again.
#[derive(Clone, Default)]
pub(crate) struct RunCache(Arc<Mutex<(u64, Places)>>);

impl fmt::Debug for RunCache {
    /// Says nothing of what is held.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RunCache")
    }
}

impl PartialEq for RunCache {
    /// What is held is worked out from the rest of the document, so it never tells two documents apart.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Document {
    /// The places of the file that can be run, each with the line it starts on, looked for again only when the text was parsed again.
    pub fn runnables(&self) -> Arc<Vec<(usize, Runnable)>> {
        let Some(syntax) = &self.syntax else {
            return Places::default();
        };
        let Ok(mut held) = self.runs.0.lock() else {
            return Places::default();
        };
        if held.0 != syntax.revision() {
            let text = self.field.text();
            let found = if text.len() <= MOST_BYTES {
                syntax
                    .runnables(text)
                    .into_iter()
                    .map(|place| (self.line_of(place.range.start), place))
                    .collect()
            } else {
                Vec::new()
            };
            *held = (syntax.revision(), Arc::new(found));
        }
        held.1.clone()
    }
}
