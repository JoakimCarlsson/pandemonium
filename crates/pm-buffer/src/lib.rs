//! The text buffer that language servers and syntax trees work on.
//!
//! A [`Buffer`] is a text with the start of each of its lines indexed, so that
//! a byte offset can be told as a line and a column the way a language server
//! counts them ([`Position`], in UTF-16 code units) or the way a syntax tree
//! does (a row and a byte column). A [`Change`] is one stretch of a text
//! replaced, found by comparing the text before and after an edit.
//!
//! [`Reach`] says how much a press takes hold of: a point, a word or a line.
//! [`word_at`] finds the word around a place in a text, the characters of
//! each [`Class`] making up one word.

mod buffer;
mod change;
mod lines;
mod position;
mod reach;
mod word;

pub use buffer::Buffer;
pub use change::Change;
pub use lines::Lines;
pub use position::{Position, Span};
pub use reach::Reach;
pub use word::{Class, word_at, word_at_with};
