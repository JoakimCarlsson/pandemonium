//! The keymap: what a keypress means, and where that is written down.
//!
//! A keypress becomes a [`Chord`], the [`Resolver`] folds it into the chords
//! pressed so far, and the [`Keymap`] in force says which [`Action`] that
//! comes to — if a [`When`] clause lets it, in the [`Context`] the window is
//! in. Several chords in a row are one binding, so `ctrl+k ctrl+o` is a
//! keybinding and not two keypresses that follow each other.
//!
//! The shape is VS Code's, because the vocabulary is the one people
//! already have: chord sequences, `when` clauses over context keys, and later
//! layers winning over earlier ones. What differs is that an action is a name
//! we know at compile time rather than any string a command registry happens
//! to hold. Every keymap on offer is a [`KeymapFile`] — the editor's own and
//! the reader's alike, [`keymaps`] lists them — and the reader's own bindings
//! are [`Changes`] laid over whichever one they chose.

mod action;
mod binding;
mod changes;
mod chord;
mod context;
mod event;
mod key;
mod offered;
mod resolver;

pub use action::{Action, Travel};
pub use binding::{BadBinding, Binding, Keymap};
pub use changes::Changes;
pub use chord::{Chord, Sequence};
pub use context::{Context, When, keys};
pub use event::chord;
pub use key::{Key, Modifiers, Named};
pub use offered::{DEFAULT_KEYMAP, KeymapFile, find, install, keymaps, name, platform, resolve};
pub use resolver::{Resolution, Resolver};
