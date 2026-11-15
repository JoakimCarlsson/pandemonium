//! Text editing models: a [`Field`] of text with a selection and the
//! [`Edit`]s that change it, how far a press reaches ([`Reach`]), and
//! [`word_at`] to find the word around a place in a text.

mod field;
mod reach;
mod word;

pub use field::{Edit, Field, Motion};
pub use reach::{Clicks, Gesture, Reach, Surface};
pub use word::{word_at, word_at_with};
