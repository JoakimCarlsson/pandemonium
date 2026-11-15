//! Sticky scroll: which lines stay at the top of a code editor's text
//! because they open the blocks the first line in view is inside of.

use pm_editor::Document;

/// The most lines that stay at the top of the view.
const MOST_STICKY: usize = 4;

/// The lines, counted from 0 and outermost first, that open the blocks
/// around line `top` of `document`, the first line in view; none at the
/// top of the file or without a syntax tree.
pub(crate) fn sticky_lines(document: &Document, top: usize) -> Vec<usize> {
    if top == 0 {
        return Vec::new();
    }
    document.sticky_lines(top, None, MOST_STICKY)
}
