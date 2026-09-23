//! The editor pane: the files the window has open, and the one it is showing.
//!
//! [`Files`] is the one seam a file is opened, edited, saved and closed
//! through; every file it holds knows the project whose worktree it was
//! opened from. Which pane shows which of them is [`crate::panes`]'s to say.
//! [`buffer_view`] is the pane itself: it draws the lines it has room for and
//! writes down what it drew as a [`TextLayout`], which is what turns a click
//! arriving later into a place in the file.

mod bar;
mod caret;
mod completions;
mod crumbs;
mod display;
mod hint;
mod keys;
mod layout;
mod menu;
mod minimap;
mod search;
mod store;
mod view;

pub use bar::search_bar;
pub use caret::Blink;
pub use completions::{Completions, completion_list};
pub use crumbs::{Crumbs, crumb_bar};
pub use display::{CursorShape, Display};
pub use hint::{Shown, code_lines, hint};
pub use keys::{Edit, edit, keystroke};
pub use menu::{TextMenu, text_menu};
pub use search::{Search, SearchField};
pub use store::{Document, FileEntry, FileId, Files, Habits, OpenFile, SCROLL_MARGIN};
pub use view::{ScrollAxis, buffer_view, plain_view, tint};
