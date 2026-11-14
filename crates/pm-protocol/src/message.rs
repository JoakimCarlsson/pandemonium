//! What goes over the connection, each message one JSON text message, possibly split across frames.

use pm_harness::Event;
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
    /// A program started in a shell serves a web page on a port of the server's machine, as the shell's output says and the port bears out.
    TerminalServing {
        /// The shell.
        terminal: u64,
        /// The address of the page, as the server's machine knows it.
        url: String,
    },
    /// A program of a session serves a web page on a port of the server's machine, found by watching the ports and which processes listen on them.
    SessionServing {
        /// The session.
        session: u64,
        /// The address of the page, as the server's machine knows it.
        url: String,
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
    /// An agent asked for a browser tool, which this client is to run in its browser and answer with `browser_result`.
    BrowserCall {
        /// The call, to be given back with the answer.
        call: u64,
        /// The session whose agent made the call; the tool acts on that session's tabs only. Zero from a server that predates it.
        #[serde(default)]
        session: u64,
        /// The tool, such as `browser_screenshot`.
        tool: String,
        /// What the agent gave it, as JSON text.
        arguments: String,
    },
    /// How far the download of an update has got. Sent to the client that asked for it.
    UpdateProgress {
        /// How many bytes have arrived.
        received: u64,
        /// How many there are in all; zero when not known.
        total: u64,
    },
    /// How continuing a chat in a project, asked for with `continue_chat`, went. Sent to the client that asked, or, when it has gone, to every client connected, so that it hears after connecting again; the other clients are told of the session with `session_added`.
    ChatContinued {
        /// The chat that was continued.
        chat: u64,
        /// The session that continues it, when one was started.
        #[serde(default)]
        started: Option<SessionInfo>,
        /// Why no session was started: the summary turn failed or was stopped, or the session could not be made.
        #[serde(default)]
        failure: Option<String>,
    },
    /// A sign-in the client asked for with `sign_in` or `sign_in_session` waits for the reader to enter `code` on the page `url`. Sent to that client alone; the answer to its request says when the sign-in is over.
    SignInCode {
        /// The agent being signed in, by its id.
        agent: String,
        /// The page to enter the code on.
        url: String,
        /// The code to enter.
        code: String,
    },
}

#[cfg(test)]
/// Tests for how the server's messages are written.
mod tests {
    use super::*;

    /// A fallback to a backup model and the slots a conversation told are written as session events and read back as they were.
    #[test]
    fn slot_events_round_trip() {
        for event in [
            Event::ModelFallback {
                slot: "main".to_owned(),
                from: "claude-opus-4-5".to_owned(),
                to: "openai/gpt-5".to_owned(),
                reason: "provider returned 529: overloaded".to_owned(),
            },
            Event::Slots(vec![pm_harness::SlotInfo {
                name: "fast".to_owned(),
                model: Some("claude/haiku:low".to_owned()),
                backups: vec!["openai/gpt-5-mini".to_owned()],
                resolved: Some("claude/haiku:low".to_owned()),
                source: pm_harness::SlotSource::User,
                inherited: false,
                suggested: None,
                providers: Vec::new(),
            }]),
        ] {
            let message = ServerMessage::Event {
                session: 2,
                seq: 7,
                event,
            };
            let text = serde_json::to_string(&message).unwrap();
            assert_eq!(
                serde_json::from_str::<ServerMessage>(&text).unwrap(),
                message
            );
        }
        let text = serde_json::to_value(Event::ModelFallback {
            slot: "plan".to_owned(),
            from: "a".to_owned(),
            to: "b".to_owned(),
            reason: "overloaded".to_owned(),
        })
        .unwrap();
        assert_eq!(text["ModelFallback"]["to"], "b");
    }
}
