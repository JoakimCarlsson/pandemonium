//! A worktree's changes as excerpts of its files, edited in one pane.
//!
//! This is Zed's multibuffer, pointed at what a session is for: reviewing
//! the work. Every changed file contributes the runs of lines it changed and
//! a few either side, one file under the next, and each excerpt is the
//! file's own open document seen through a window — typing in it is typing
//! in the file, saving it saves the file, and a language server hears of it
//! the way it hears of any other edit. What the lines are compared against
//! is the last commit, so a change the agent staged is as much in view as
//! one it did not.
//!
//! [`Excerpts`] is the model: which files, which of them holds the cursor,
//! and where the pane is scrolled to. [`excerpts_view`] is the pane.

mod store;
mod view;

pub use store::{Excerpted, Excerpts, OpenExcerpts};
pub use view::excerpts_view;
