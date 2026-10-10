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
//! Preferences open in a modal above the workspace and belong to the window,
//! whichever project or session is in front.

mod agent_list;
mod agents;
mod form;
mod language_settings;
pub mod languages;
mod mcp;
mod page;
mod parts;
mod state;

pub use agent_list::{AgentCatalog, AgentList};
pub use form::{FormField, ServerForm, Subject};
pub use mcp::{Catalog, McpPage, suggestions};
pub use page::{SettingsPane, settings_modal, settings_pane};
pub use state::{Settings, SettingsPage, SettingsSection};
