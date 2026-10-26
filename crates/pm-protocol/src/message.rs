//! What goes over the connection, each message one JSON text message, possibly split across frames.

use pm_acp::Event;
use serde::{Deserialize, Serialize};

use crate::{Request, Response, SessionInfo};

/// A message from a client to the server.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
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
    /// Another client closed a session. Sent to every client but the one that closed it.
    SessionClosed {
        /// The session.
        session: u64,
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
    /// What a shell printed, which a client feeds to its emulator.
    TerminalOutput {
        /// The shell.
        terminal: u64,
        /// How many bytes the shell had printed before these.
        offset: u64,
        /// The bytes.
        #[serde(with = "crate::bytes")]
        data: Vec<u8>,
    },
    /// A shell ended.
    TerminalExited {
        /// The shell.
        terminal: u64,
    },
    /// What a language server finds wrong with a file the connection opened, replacing what it found before.
    Diagnostics {
        /// The folder the file was opened in.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text the diagnostics are about, when the server says.
        version: Option<i32>,
        /// What is wrong.
        diagnostics: Vec<pm_lsp::Diagnostic>,
    },
    /// The files of a folder the connection watches, or what git keeps about them, changed. Changes coming close together are told once.
    FolderChanged {
        /// The folder, as the connection named it.
        directory: String,
    },
    /// A debugging session moved on: what it looks like now, and what the window should act on.
    Debug {
        /// The session.
        debug: u64,
        /// What it looks like now.
        view: crate::DebugView,
        /// What happened since the last message, oldest first.
        events: Vec<crate::DebugEvent>,
    },
    /// The extensions changed: one was installed or removed, or installing one moved on. Sent to every client.
    ExtensionsChanged {
        /// Every extension, as `extensions` lists them.
        extensions: Vec<crate::ExtensionInfo>,
    },
    /// How far the download of an update has got. Sent to the client that asked for it.
    UpdateProgress {
        /// How many bytes have arrived.
        received: u64,
        /// How many there are in all; zero when not known.
        total: u64,
    },
}
