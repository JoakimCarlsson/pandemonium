//! A question the window asks before doing something it cannot take back.
//!
//! Throwing a change away and taking a file off the disk are not things to be
//! quietly wrong about, so each of them is asked about first, in the one place
//! the window asks: a modal card with the answers on it. [`Prompt`] is the
//! question, [`prompt`] is the card, and what the answer does is a [`Message`]
//! like any other.
//!
//! [`Message`]: crate::message::Message

mod state;
mod view;

pub use state::{Answer, Prompt};
pub use view::{WIDTH, prompt};
