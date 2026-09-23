//! Pictures opened from a worktree: the files a text editor cannot read.
//!
//! [`Images`] is the one seam a picture is opened, read again and closed
//! through, the way [`crate::editor::Files`] is for text; every picture it
//! holds knows the worktree it was opened from. [`image_pane`] is the pane a
//! picture is looked at in.

mod pane;
mod store;

pub use pane::image_pane;
pub use store::{ImageId, Images, Shown};
