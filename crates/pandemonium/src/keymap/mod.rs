//! The keymap: what a keypress means, and where that is written down.
//!
//! A keypress becomes a [`Chord`], the [`Resolver`] folds it into the chords
//! pressed so far, and the [`Keymap`] in force says which [`Action`] that
//! comes to — if a [`When`] clause lets it, in the [`Context`] the window is
//! in. Several chords in a row are one binding, so `ctrl+k ctrl+o` is a
//! keybinding and not two keypresses that follow each other.
//!
//! The shape is VS Code's, because the vocabulary is the one people already
//! have: chord sequences, `when` clauses over context keys, and later layers
//! winning over earlier ones. What differs is that an action is a name we
//! know at compile time rather than any string a command registry happens to
//! hold, and that every keymap the editor ships is one base table plus an
//! overlay, in [`tables`].

mod action;
mod base;
mod binding;
mod chord;
mod context;
mod event;
mod key;
mod resolver;
mod tables;

pub use action::Action;
pub use base::BaseKeymap;
pub use binding::{Binding, Keymap};
pub use chord::{Chord, Sequence};
pub use context::{Context, When, keys};
pub use event::chord;
pub use key::{Key, Modifiers, Named};
pub use resolver::{Resolution, Resolver};
