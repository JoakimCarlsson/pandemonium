//! Talking to an agent: one conversation, drawn as a pane.
//!
//! A [`Talk`] is one agent the window has running in a worktree, spoken to
//! over the Agent Client Protocol. The worktree it works in is a
//! [`pm_core::Session`]; this is what is said in it. [`Talks`] is the model —
//! the one seam an agent is started and ended through, keyed by project id
//! the way the shells are; [`agent_pane`] is the screen it is read and
//! written in, and it is a pane like any other, splittable and tabbable
//! beside the files.
//!
//! What the agent says arrives a fragment at a time; [`Transcript`] is where
//! those fragments become the conversation as it now stands.

mod form;
mod pane;
mod store;
mod transcript;

pub use form::Form;
pub use pane::{
    agent_pane, content_height, everything, lines_between, selected_text, standing_color,
    words_between,
};
pub use store::{Pasted, Standing, Talk, TalkId, Talks, Tally};
pub use transcript::Block;

pub use pm_ui::Spot;
