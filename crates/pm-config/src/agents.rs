//! What the window does when an agent finishes its turn.

use serde::{Deserialize, Serialize};

/// What the window does with a session whose agent finished its turn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Finish {
    /// Nothing beyond marking it in the sidebar.
    Nothing,
    /// Tells the reader, as the notification setting says.
    #[default]
    Notify,
    /// Opens the session on what it changed.
    OpenReview,
}

/// How the window tells the reader an agent finished.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Notification {
    /// A sound.
    Sound,
    /// A notification of the desktop's own.
    #[default]
    Desktop,
    /// Neither.
    None,
}

/// What the window does when an agent finishes its turn.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentSettings {
    /// What it does with the session.
    pub on_finish: Finish,
    /// How it tells the reader.
    pub notification: Notification,
}
