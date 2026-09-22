//! Agent sessions: an agent working in a worktree, drawn as a pane.
//!
//! A session is one agent the window has running over one project's worktree,
//! spoken to over the Agent Client Protocol. [`Sessions`] is the model — the
//! one seam an agent is started and ended through, keyed by project id the way
//! the shells are; [`agent_pane`] is the screen it is read and written in, and
//! it is a pane like any other, splittable and tabbable beside the files.
//!
//! What the agent says arrives a fragment at a time; [`Transcript`] is where
//! those fragments become the conversation as it now stands.

mod pane;
mod store;
mod transcript;

pub use pane::{agent_pane, row_count};
pub use store::{SessionId, Sessions, Talk};
pub use transcript::Block;
