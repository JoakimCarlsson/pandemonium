//! A worktree's live tree of declarations in its last focused file.

mod pane;
mod store;

pub use pane::{ROW_HEIGHT, outline_pane};
pub use store::{Outline, Store};
