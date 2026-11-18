//! The text of a field being edited, apart from how it is drawn.
//!
//! A [`Field`] holds a text with its carets, selections, composition and
//! undo history, and changes by one [`Edit`] at a time. [`Reach`] says how
//! much a press takes hold of, [`Clicks`] and [`Gesture`] turn presses and
//! drags into selections over a [`Surface`], and [`word_at`] finds the word
//! around a place in a text.

mod field;
mod reach;
mod word;

pub use field::{Edit, Field, Motion};
pub use reach::{Clicks, Gesture, Reach, Surface};
pub use word::{word_at, word_at_with};
