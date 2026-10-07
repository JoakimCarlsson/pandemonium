//! Durable relationships between native provider conversations.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A native conversation fork, sharing the source's current filesystem.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConversationFork {
    /// The adapter that owns both provider identities.
    pub agent: String,
    /// The provider identity whose context was copied.
    pub source: String,
    /// The last retained provider message, absent for a whole-session fork.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// The independently promptable destination provider identity.
    pub destination: String,
    /// The shared filesystem root; no checkpoint or rewind was applied.
    pub shared_root: PathBuf,
}
