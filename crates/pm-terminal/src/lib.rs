//! The terminal emulator: escape-sequence parsing, the cell grid and scrollback.
//!
//! An [`Emulator`] is fed the bytes a shell prints and keeps what a terminal
//! window shows: a grid of [`Cell`]s, the lines scrolled off its top and the
//! modes the program running in it has switched on. It draws nothing itself.

mod cell;
mod emulator;
mod grid;
mod palette;

pub use cell::{Cell, Color, Flags};
pub use emulator::{Emulator, Link, Selection};
pub use palette::{ANSI, indexed};
