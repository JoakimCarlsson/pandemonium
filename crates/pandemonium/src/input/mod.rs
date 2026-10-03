//! The box text is written in, wherever the window asks for one.
//!
//! A commit message and an agent's prompt are the same thing twice: a buffer
//! with a border round it, a caret while it has the keyboard, a menu under
//! the right button, and one key that finishes it. [`Input`] is that thing —
//! what is in the box and what the keyboard does to it — and [`input_view`]
//! is how it is drawn. A screen says how tall the box is, what finishes it
//! and what that means; it does not say how a box behaves.

mod state;
mod view;

pub use state::{Input, Submit};
pub use view::{bare_input_view, input_menu, input_view, text_view};
