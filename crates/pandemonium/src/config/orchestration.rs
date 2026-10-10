//! Persisted limits and explicit project grants for editor-owned tools.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Reader-configured limits for autonomous delegation and messaging.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Orchestration {
    /// Maximum child depth; zero disables delegation.
    pub max_depth: usize,
    /// Maximum live delegated worktrees per parent; zero disables delegation.
    pub max_children: usize,
    /// Maximum delegated worktrees across the window, including pending creations.
    pub max_sessions: usize,
    /// Maximum messages per calling conversation per editor launch.
    pub max_messages: usize,
    /// Maximum pending follow-ups in any conversation.
    pub max_pending_messages: usize,
    /// Maximum returned text characters, additionally capped at 16384.
    pub max_context_chars: usize,
    /// Explicit source project roots and the target project roots they may address.
    pub project_grants: BTreeMap<PathBuf, Vec<PathBuf>>,
}

impl Default for Orchestration {
    /// Conservative defaults that allow small delegation trees.
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_children: 4,
            max_sessions: 16,
            max_messages: 50,
            max_pending_messages: 8,
            max_context_chars: 8192,
            project_grants: BTreeMap::new(),
        }
    }
}
