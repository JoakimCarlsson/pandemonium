//! Reviewing what a project has changed, and doing something about it.
//!
//! [`Review`] is the model: what git last said about one worktree and each
//! [`Repository`] in it, the lines behind each of those files, and the one
//! seam the window stages, throws away and commits through. The two screens built from it are the sidebar
//! that lists the changes and the pane the changes are read in — one thing
//! seen twice, never two stores.

mod action;
pub(crate) mod comment;
pub(crate) mod conflict;
mod editor;
mod graph;
mod gutter;
mod pane;
mod reading;
mod remark;
mod repository;
mod scroll;
mod shade;
mod sidebar;
mod store;
mod work;

pub use action::RepositoryAction;
pub use conflict::Action as ConflictAction;
pub use editor::commit_editor;
pub use pane::{
    Remarking, change_pane, hunk_anchor, hunk_row, line_at, review_pane, row_count, row_of,
};
pub use reading::Reading;
pub use remark::{Delivery, block_rows, comment_block, composer_block, composer_rows};
pub use sidebar::{SourceControlControls, change_menu, changes_sidebar, status_color};
pub use store::{ChangeId, Group, Review, StashAction};
pub use work::{Done, Work};

pub(crate) use pane::{TurnDiff, step_prefix, turns_pane};
