//! The terminal pane: a shell in the selected worktree, drawn as a grid.
//!
//! [`Terminals`] is the one seam a shell is started and stopped through, and
//! it is keyed by project id, so the shell a pane shows is the shell of the
//! worktree the window is pointed at — the same rule the file tree follows.
//! [`terminal_view`] is the pane itself: it draws one [`pm_vt::Terminal`] and
//! tells it how many columns and rows the space it was given comes to.

mod keys;
mod store;
mod view;

pub use keys::{key, modifiers};
pub use store::{Shell, ShellEntry, ShellId, Terminals};
pub use view::terminal_view;
