//! The pane tree: how the window is divided, and how that division is drawn.
//!
//! [`PaneTree`] is the model — panes, the splits between them and which pane
//! has the keyboard — and [`pane_tree`] is the screen built from it. Nothing
//! here holds a document: a pane names what is open in it, as an [`Item`],
//! and the stores those items point into are where the things themselves
//! live.

mod arrangement;
mod item;
mod saved;
mod tool;
mod tree;
mod view;

pub use arrangement::Arrangement;
pub use item::{Item, Role};
pub use saved::{Saved, SavedAxis, SavedKind, SavedNode, SavedTab};
pub use tool::Tool;
pub use tree::{Pane, PaneId, PaneTree, SplitDirection, SplitId};
pub use view::{Content, Contents, Shortcut, TabEntry, pane_tree, tab_menu, unsaved_menu};
