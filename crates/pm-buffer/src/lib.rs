//! The text buffer that language servers and syntax trees work on.
//!
//! A [`Buffer`] is a text with the start of each of its lines indexed, so that
//! a byte offset can be told as a line and a column the way a language server
//! counts them ([`Position`], in UTF-16 code units) or the way a syntax tree
//! does (a row and a byte column). A [`Change`] is one stretch of a text
//! replaced, found by comparing the text before and after an edit.

mod buffer;
mod change;
mod lines;
mod position;

pub use buffer::Buffer;
pub use change::Change;
pub use lines::Lines;
pub use position::{Position, Span};
