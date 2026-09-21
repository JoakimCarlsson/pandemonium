//! The pane tree: how the window is divided, and how that division is drawn.
//!
//! [`PaneTree`] is the model — panes, the splits between them and which pane
//! has the keyboard — and [`pane_tree`] is the screen built from it. Nothing
//! here holds a document: a pane names the files open in it, and
//! [`crate::editor::Files`] is where those files live.

mod saved;
mod tree;
mod view;

pub use saved::{Saved, SavedTab};
pub use tree::{Pane, PaneId, PaneTree, SplitDirection, SplitId};
pub use view::{Contents, file_menu, pane_menu, pane_tree};
