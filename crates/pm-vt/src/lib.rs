//! Terminal emulation: the pty, the escape-sequence parser and the cell grid.
//!
//! A [`Terminal`] is a child process in a pseudoterminal, the bytes it writes
//! and the screen those bytes draw. It drives a plain shell and an agent CLI
//! alike, and knows about neither: the caller says what to run, pumps the
//! terminal when it is woken and draws the [`Grid`] that comes back.
//!
//! The crate root is the facade and nothing else: every type lives in the
//! module that owns it, and this file only says which ones callers may name.

mod cell;
mod color;
mod emulator;
mod grid;
mod keys;
mod modes;
mod pty;
mod sgr;
mod terminal;

pub use cell::{Attrs, Cell, Color};
pub use color::palette;
pub use grid::{Cursor, Grid, Line};
pub use keys::{Key, Modifiers, encode, paste};
pub use modes::Modes;
pub use pty::Notify;
pub use terminal::Terminal;

pub use portable_pty::CommandBuilder;
