//! The editor pane: the files the window has open, and the one it is showing.
//!
//! [`Files`] is the one seam a file is opened, edited, saved and closed
//! through; every file it holds knows the project whose worktree it was
//! opened from. Which pane shows which of them is [`crate::panes`]'s to say.
//! [`buffer_view`] is the pane itself: it draws the lines it has room for
//! and tells the document how many that was.

mod keys;
mod store;
mod view;

pub use keys::{Edit, edit};
pub use store::{Document, FileEntry, FileId, Files, OpenFile};
pub use view::buffer_view;
