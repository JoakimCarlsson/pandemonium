//! The settings pane: every preference the editor keeps, a page at a time.
//!
//! Settings is one of the two editors of [`crate::config::Preferences`] —
//! onboarding is the other — and it keeps nothing of them itself. A row reads
//! the value it shows from the preferences and answers with the same
//! [`crate::message::Message`] onboarding does, so the one seam that changes a
//! preference, and the one file it is written to, are the same whichever
//! screen the reader used. What the pane does remember is where the reader
//! is in it: the page, and how far down that page.
//!
//! The pane is a tab like any other, which belongs to the window rather than
//! to a worktree: it is drawn whichever project or session is in front.

mod mcp;
mod page;
mod state;

pub use mcp::{Catalog, McpPage};
pub use page::{SettingsPane, settings_pane};
pub use state::{Settings, SettingsPage, SettingsSection};
