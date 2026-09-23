//! Modal editing: vim's keys over a `pm-text` buffer.
//!
//! A [`Vim`] takes keys one at a time and carries them out on a buffer —
//! motions, operators, text objects, registers, counts, `.` and macros —
//! keeping each buffer's mode in its own [`State`]. It knows nothing of the
//! window: the window turns its key events into [`Keystroke`]s, types the
//! keys insert mode hands back to it, and carries out the [`Effect`]s a key
//! asks for, such as writing the file or splitting the pane.
//!
//! The crate root is the facade and nothing else: every type lives in the
//! module that owns it, and this file only says which ones callers may name.

mod command;
mod key;
mod mode;
mod motion;
mod object;
mod operator;
mod register;
mod search;
mod text;
mod vim;

pub use command::{Placement, Window};
pub use key::{Key, Keystroke};
pub use mode::{Mode, Shape};
pub use motion::View;
pub use register::Clipboard;
pub use vim::{Effect, Outcome, State, Vim};
