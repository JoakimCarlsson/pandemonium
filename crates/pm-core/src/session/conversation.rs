//! Durable relationships between native provider conversations.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A whole-session native fork, sharing the source's current filesystem.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConversationFork {
    /// The adapter that owns both provider identities.
    pub agent: String,
    /// The provider identity whose context was copied.
    pub source: String,
    /// The independently promptable destination provider identity.
    pub destination: String,
    /// The shared filesystem root; no checkpoint or rewind was applied.
    pub shared_root: PathBuf,
}
