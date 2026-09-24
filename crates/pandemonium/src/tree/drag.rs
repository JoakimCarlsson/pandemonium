//! Rows of the tree carried across it, and where letting go puts them.

use std::path::PathBuf;

use pm_gfx::Point;

use crate::panes::PaneId;

/// How far a press may travel and still have been a click on the row.
const SLIP: f32 = 4.0;

/// Rows under the pointer, and where letting go of them would put them.
#[derive(Clone, Debug)]
pub struct EntryDrag {
    /// The row the press landed on.
    pub pressed: PathBuf,
    /// Everything carried: the selection when the row was part of it.
    pub paths: Vec<PathBuf>,
    /// Where the pointer is now.
    pub at: Point,
    /// How far the pointer has travelled since the press.
    pub travelled: f32,
    /// The directory the rows would land in, if the pointer is over one.
    pub target: Option<PathBuf>,
    /// The pane they would open in, if the pointer is over one.
    pub pane: Option<PaneId>,
    /// Whether the press left the selection for the release to settle.
    ///
    /// Pressing a row that is already part of a larger selection keeps the
    /// selection, so that all of it can be carried; if the press turns out
    /// to be a click, the release selects that row alone.
    pub deferred: bool,
    /// Whether the press had no modifier held, so that as a click it opens.
    pub plain: bool,
}

impl EntryDrag {
    /// Whether the pointer has gone far enough to have carried the rows.
    pub fn is_carried(&self) -> bool {
        self.travelled > SLIP
    }
}
