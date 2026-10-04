//! Durable ancestry of independently reviewable delegated worktrees.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The caller that delegated a session, identified by durable filesystem roots.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Delegation {
    /// The project that owns the parent conversation.
    pub project: PathBuf,
    /// The parent's checkout or independent session worktree.
    pub parent: PathBuf,
    /// The parent's provider conversation, where one was advertised.
    pub conversation: Option<String>,
    /// The parent session name retained if the parent is later removed.
    pub name: String,
    /// The number of delegation edges from a reader-started conversation.
    pub depth: usize,
}
