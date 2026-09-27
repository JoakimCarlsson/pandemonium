//! What a buffer has already worked out about its text as it stands.
//!
//! A pane asks the same questions of a buffer every frame — how the lines on
//! screen are coloured, which bracket answers the one at the cursor, which
//! lines the top of the pane is inside — and between two keystrokes the
//! answers do not change. Each is kept here beside the version of the text
//! it was worked out at, and worked out again only once that version has
//! gone.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::sync::Arc;

use crate::cursor::Position;
use crate::syntax::Highlights;

/// How many ranges of lines the highlights are kept for at once.
///
/// One for the text of each pane showing the buffer and one for its
/// minimap, with room for a review excerpt beside them.
const HIGHLIGHT_SLOTS: usize = 4;

/// The brackets matched at a cursor, and the version and cursor they were
/// matched at.
type Brackets = (i32, Position, Option<(Position, Position)>);

/// The lines holding a line, and the version, the line and the tab width
/// they were found at.
type Enclosing = (i32, usize, usize, Vec<usize>);

/// The answers a buffer has worked out, each with what it was worked out for.
#[derive(Default)]
pub(super) struct Memo {
    /// The highlights of a range of lines, by the version they were read at.
    highlights: Vec<(i32, Range<usize>, Arc<Highlights>)>,
    /// The brackets matched at a cursor, by the version and the cursor.
    brackets: Cell<Option<Brackets>>,
    /// The lines holding a line, by the version, the line and the tab width.
    enclosing: RefCell<Option<Enclosing>>,
}

impl Memo {
    /// The highlights of `lines` at `version`, worked out by `work` unless
    /// they already were.
    pub(super) fn highlights(
        &mut self,
        version: i32,
        lines: Range<usize>,
        work: impl FnOnce() -> Highlights,
    ) -> Arc<Highlights> {
        self.highlights.retain(|(kept, _, _)| *kept == version);
        if let Some((_, _, found)) = self.highlights.iter().find(|(_, kept, _)| *kept == lines) {
            return found.clone();
        }
        let found = Arc::new(work());
        if self.highlights.len() >= HIGHLIGHT_SLOTS {
            self.highlights.remove(0);
        }
        self.highlights.push((version, lines, found.clone()));
        found
    }

    /// Forgets every highlight, for when what colours the text has changed
    /// without the text changing.
    pub(super) fn forget_highlights(&mut self) {
        self.highlights.clear();
    }

    /// The brackets matched at `head` at `version`, worked out by `work`
    /// unless they already were.
    pub(super) fn brackets(
        &self,
        version: i32,
        head: Position,
        work: impl FnOnce() -> Option<(Position, Position)>,
    ) -> Option<(Position, Position)> {
        if let Some((kept, at, found)) = self.brackets.get()
            && kept == version
            && at == head
        {
            return found;
        }
        let found = work();
        self.brackets.set(Some((version, head, found)));
        found
    }

    /// The lines holding `line` at `version` with tabs `tab` wide, worked
    /// out by `work` unless they already were.
    pub(super) fn enclosing(
        &self,
        version: i32,
        line: usize,
        tab: usize,
        work: impl FnOnce() -> Vec<usize>,
    ) -> Vec<usize> {
        if let Some((_, _, _, found)) = self
            .enclosing
            .borrow()
            .as_ref()
            .filter(|(kept, at, width, _)| (*kept, *at, *width) == (version, line, tab))
        {
            return found.clone();
        }
        let found = work();
        *self.enclosing.borrow_mut() = Some((version, line, tab, found.clone()));
        found
    }
}
