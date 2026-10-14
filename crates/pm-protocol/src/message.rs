//! What goes over the connection, each message one frame of text.

use pm_acp::Event;
use serde::{Deserialize, Serialize};

use crate::{Request, Response, SessionInfo};

/// A message from a client to the server.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// The first message of a connection: which protocol the client speaks and the token that lets it in.
    Hello {
        /// The protocol version.
        version: u32,
        /// The token the server wants, when it wants one.
        token: Option<String>,
    },
    /// A request, to be answered by the response carrying the same `id`.
    Request {
        /// What the answer will be matched to.
        id: u64,
        /// What is asked.
        request: Request,
    },
}

/// A message from the server to a client.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// The client was let in.
    Ready {
        /// The protocol version the server speaks.
        version: u32,
        /// The version of the server program.
        server: String,
    },
    /// The client was turned away, for this reason.
    Denied {
        /// Why.
        reason: String,
    },
    /// The answer to the request with this `id`.
    Response {
        /// The request it answers.
        id: u64,
        /// The answer.
        response: Response,
    },
    /// Another client started a session. Sent to every client but the one that started it.
    SessionAdded {
        /// What the server knows about the session.
        info: SessionInfo,
    },
    /// Another client changed the projects. Sent to every client but the one that changed them.
    ProjectsChanged {
        /// The projects now, oldest first.
        paths: Vec<String>,
    },
    /// Something a session's agent did, numbered so a client can ask for what it missed.
    Event {
        /// The session.
        session: u64,
        /// Counted from one for each session.
        seq: u64,
        /// What happened.
        event: Event,
    },
}
