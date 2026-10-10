//! Notebook pane state and controls over the shared file, text and image paths.

mod pane;
mod store;

pub use pane::notebook_pane;
pub use store::{Action, Notebooks};
