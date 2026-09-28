//! Markdown read as it renders, beside the text it is rendered from.
//!
//! A rendered document is a pane of its own onto an open file: the file is
//! edited in its own pane and this one follows it keystroke by keystroke,
//! because it is drawn from the same document every frame. [`Renders`] keeps
//! what drawing it needs between frames — how far each is scrolled, the
//! blocks the text last parsed into and the pictures it names.

pub(crate) mod blocks;
mod diagram;
mod pane;
mod store;

pub use pane::rendered_pane;
pub use store::{Renders, is_markdown};
