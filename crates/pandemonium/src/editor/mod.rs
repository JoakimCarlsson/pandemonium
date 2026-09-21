//! The editor pane: the files the window has open, and the one it is showing.
//!
//! [`Files`] is the one seam a file is opened, edited, saved and closed
//! through, keyed by project id so that a file belongs to the worktree it
//! was opened from. [`buffer_view`] is the pane itself: it draws the lines
//! it has room for and tells the document how many that was.

mod keys;
mod store;
mod view;

pub use keys::{Edit, edit};
pub use store::{Document, FileEntry, FileId, Files, OpenFile};
pub use view::buffer_view;
