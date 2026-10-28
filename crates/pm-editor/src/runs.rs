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

/// A link at the end of a line that runs a place of the file with one task of its language.
#[derive(Clone, Debug, PartialEq)]
pub struct RunLensItem {
    /// The line the place starts on.
    pub line: usize,
    /// What the link says, and what the task is called.
    pub label: String,
    /// The place it runs.
    pub place: Runnable,
}

impl Document {
    /// Whether the language shows `place` as links at the end of its line instead of a run button.
    pub fn is_lens_place(&self, place: &Runnable) -> bool {
        self.editing
            .run_lenses
            .iter()
            .any(|(tags, _)| tags.iter().any(|tag| place.tags.contains(tag)))
    }

    /// Whether the language puts links on a row of their own above any line of the file, or a language server gave lenses, which go there too.
    pub fn has_lens_rows(&self) -> bool {
        self.has_code_lenses()
            || (!self.editing.run_lenses.is_empty()
                && self
                    .runnables()
                    .iter()
                    .any(|(_, place)| self.is_lens_place(place)))
    }

    /// Whether the line at `index` has links on a row of its own above it: those that run its place, and the lenses a language server gave for it.
    pub fn lens_row_at(&self, index: usize) -> bool {
        !self.lenses_on(index).is_empty()
            || (!self.editing.run_lenses.is_empty()
                && self
                    .runnables()
                    .iter()
                    .any(|(line, place)| *line == index && self.is_lens_place(place)))
    }

    /// The links the language shows above lines, in order of the text; the place of a link in this list is what names it when it is pressed.
    pub fn run_lens_items(&self) -> Vec<RunLensItem> {
        if self.editing.run_lenses.is_empty() {
            return Vec::new();
        }
        let mut found = Vec::new();
        for (line, place) in self.runnables().iter() {
            for (tags, label) in &self.editing.run_lenses {
                if tags.iter().any(|tag| place.tags.contains(tag)) {
                    found.push(RunLensItem {
                        line: *line,
                        label: label.clone(),
                        place: place.clone(),
                    });
                }
            }
        }
        found
    }
}
