//! Editor-owned orchestration coordinated through the session and agent seams.

mod lifecycle;
mod links;
mod tools;

use crate::agent::TalkId;
use crate::orchestration::{Call, Server};
use pm_acp::Agent;
use pm_core::{Delegation, ProjectId, SessionId};
use std::collections::BTreeMap;
use std::time::Instant;

/// The server and bounded operations still owned by this window.
#[derive(Default)]
pub(super) struct Orchestration {
    /// The loopback transport, unavailable only when binding failed.
    server: Option<Server>,
    /// Child creations reserved before any filesystem work begins.
    pending: BTreeMap<u64, Pending>,
    /// Failed creations waiting for their worktree teardown before replying.
    rollback: BTreeMap<SessionId, (Call, String)>,
    /// Messages accepted from each caller during this editor launch.
    messages: BTreeMap<TalkId, usize>,
    /// The next creation ticket for background filesystem work.
    next: u64,
}

/// A child being cut or waiting for its negotiated ACP conversation.
struct Pending {
    /// The invocation to answer once creation is ready or rolled back.
    call: Call,
    /// The independently reviewable target project.
    project: ProjectId,
    /// Durable ancestry of the calling conversation.
    parent: Delegation,
    /// The advertised adapter selected for the child.
    agent: Agent,
    /// The bounded first task, without implicit git publication permission.
    prompt: String,
    /// An advertised model requested for the selected adapter.
    model: Option<String>,
    /// Whether the requested model is awaiting adapter confirmation.
    model_requested: bool,
    /// The worktree once the background cut has completed.
    session: Option<SessionId>,
    /// The ACP conversation once its process has started.
    talk: Option<TalkId>,
    /// When the creation reservation was made.
    started: Instant,
}
