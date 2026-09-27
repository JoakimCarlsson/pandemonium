//! Pictures opened from a worktree: the files a text editor cannot read.
//!
//! [`Images`] is the one seam a picture is opened, read again and closed
//! through, the way [`crate::editor::Files`] is for text; every picture it
//! holds knows the worktree it was opened from. [`image_pane`] is the pane a
//! picture is looked at in, and [`Decodes`] is how every picture the window
//! draws is decoded away from it.

mod decode;
mod pane;
mod store;

pub use decode::{Decodes, Decoding, read_file, wake_with};
pub use pane::image_pane;
pub use store::{ImageId, Images, Shown};
