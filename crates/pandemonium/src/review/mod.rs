//! Reviewing what a project has changed, and doing something about it.
//!
//! [`Review`] is the model: what git last said about one worktree, the lines
//! behind each of those files, and the one seam the window stages, throws
//! away and commits through. The two screens built from it are the sidebar
//! that lists the changes and the pane the changes are read in — one thing
//! seen twice, never two stores.

mod editor;
mod pane;
mod sidebar;
mod store;

pub use editor::commit_editor;
pub use pane::{change_pane, hunk_row, review_pane, row_count, row_of};
pub use sidebar::{SourceControlControls, change_menu, changes_sidebar, status_color};
pub use store::{ChangeId, Group, Review};
