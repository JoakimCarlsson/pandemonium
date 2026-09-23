//! Modal editing: vim's keys over a `pm-text` buffer, bound the way Zed's
//! vim mode binds them.
//!
//! A [`Vim`] takes keys one at a time, looks each up in a binding table
//! written like Zed's `vim.json`, and carries the action it names out on a
//! buffer — motions, operators, text objects, registers, counts, `.`,
//! macros, visual, block and replace modes, the command line — keeping
//! each buffer's mode in its own [`State`]. It knows nothing of the window:
//! the window turns its key events into [`Keystroke`]s, types the keys
//! insert mode hands back to it, and carries out the [`Effect`]s a key asks
//! for, such as writing the file, splitting the pane or going to a
//! definition.
//!
//! The crate root is the facade and nothing else: every type lives in the
//! module that owns it, and this file only says which ones callers may name.

mod action;
mod engine;
mod format;
mod key;
mod keymap;
mod mode;
mod motion;
mod object;
mod operator;
mod register;
mod search;
mod surround;
mod syntax;
mod text;

pub use action::{Command, Placement};
pub use engine::{Effect, Outcome, State, Vim};
pub use key::{Key, Keystroke};
pub use keymap::BadBinding;
pub use mode::{Mode, Shape};
pub use motion::View;
pub use register::{Clipboard, ClipboardUse};
