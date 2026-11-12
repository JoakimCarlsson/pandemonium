//! Tests of continuing a chat in a project: what the summary turn comes to, and what the client that asked is told.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use pm_harness::Event;

use super::*;
use crate::sessions::Meta;

/// The number the chat of these tests goes by, which no real session has.
const CHAT: u64 = 4_000_000_501;

/// Removes what a test left of the chat numbered `CHAT` in the app's data folder when it goes out of scope, even when the test fails.
struct Forget;

impl Drop for Forget {
    /// Removes the folder of the session numbered `CHAT`.
    fn drop(&mut self) {
        if let Some(root) = pm_store::data_directory() {
            let _ = std::fs::remove_dir_all(root.join("sessions").join(CHAT.to_string()));
        }
    }
}

/// A client, and what the server sent it.
fn client() -> (Arc<Connection>, Receiver<ServerMessage>) {
    let (outbox, inbox) = channel();
    let connection = Arc::new(Connection {
        outbox: crate::core::Outbox::new(outbox),
        attached: Mutex::new(HashSet::new()),
    });
    (connection, inbox)
}

/// A chat of the first agent with no agent running, being continued in a project for `asker`, its agent just asked for the summary.
fn chat(asker: &Arc<Connection>) -> Session {
    let meta = Meta {
        agent: pm_harness::agents()[0].id.to_owned(),
        repository: String::new(),
        directory: String::new(),
        branch: None,
        head: None,
        base: None,
        title: "Pick a parser".to_owned(),
        closed: false,
        updated: 0,
        login: None,
        submodule: None,
        container: false,
        chat: true,
    };
    let mut session = Session::new(pm_harness::agents()[0], PathBuf::new(), Vec::new(), meta);
    session.handoff = Some(Handoff {
        repository: "/work/app".to_owned(),
        base: "main".to_owned(),
        title: "Pick a parser".to_owned(),
        asker: asker.clone(),
        outcome: None,
    });
    session.note(
        CHAT,
        &Event::Prompted {
            text: PROMPT.to_owned(),
            files: Vec::new(),
            checkpoint: false,
            turn: 1,
        },
    );
    session.busy = true;
    session
}

/// A tool the agent used, under `id`.
fn tool(id: &str) -> Event {
    Event::Tool(pm_harness::Tool {
        id: id.to_owned(),
        title: None,
        kind: None,
        state: None,
        target: None,
        diff: None,
        output: None,
        parent: None,
        agent: None,
        worktree: None,
    })
}

/// The summary is what the agent said after the last tool it used, and is kept once the turn ends cleanly.
#[test]
fn the_summary_is_the_agents_final_message() {
    let _forget = Forget;
    let (asker, _inbox) = client();
    let mut session = chat(&asker);
    for event in [
        Event::Said("Let me look at the notes first.".to_owned()),
        tool("read"),
        Event::Said("The goal is a parser.".to_owned()),
        Event::Started(pm_harness::Part::Text),
        Event::Said("Next: add the crate.".to_owned()),
    ] {
        session.take_in(CHAT, event);
    }
    assert!(
        session
            .handoff
            .as_ref()
            .is_some_and(|handoff| handoff.outcome.is_none()),
        "nothing is settled while the turn runs"
    );

    session.take_in(CHAT, Event::Finished(None));

    let outcome = session.handoff.and_then(|handoff| handoff.outcome);
    assert_eq!(
        outcome,
        Some(Ok(
            "The goal is a parser.\n\nNext: add the crate.".to_owned()
        ))
    );
}

/// A summary turn that was stopped starts no session, and the client that asked is told why.
#[test]
fn a_stopped_summary_starts_no_session() {
    let _forget = Forget;
    let (asker, inbox) = client();
    let mut session = chat(&asker);
    for event in [
        Event::Said("The goal is".to_owned()),
        Event::Halted,
        Event::Finished(None),
    ] {
        session.take_in(CHAT, event);
    }
    let core = Core::new();
    core.sessions
        .lock()
        .expect("sessions")
        .insert(CHAT, session);

    deliver(&core);

    let told = inbox
        .recv_timeout(Duration::from_secs(5))
        .expect("the client is told");
    assert_eq!(
        told,
        ServerMessage::ChatContinued {
            chat: CHAT,
            started: None,
            failure: Some("the summary was stopped".to_owned()),
        }
    );
    let sessions = core.sessions.lock().expect("sessions");
    assert_eq!(sessions.len(), 1, "no session was started");
    assert!(sessions[&CHAT].handoff.is_none());
}

/// When the client that asked has gone, the clients connected are told how it went, so that it hears after connecting again.
#[test]
fn a_client_that_left_is_told_through_the_others() {
    let _forget = Forget;
    let (asker, inbox) = client();
    drop(inbox);
    let mut session = chat(&asker);
    session.take_in(CHAT, Event::Failed("the agent crashed".to_owned()));
    let core = Core::new();
    core.sessions
        .lock()
        .expect("sessions")
        .insert(CHAT, session);
    let (again, heard) = client();
    core.connections.lock().expect("connections").push(again);

    deliver(&core);

    let told = heard
        .recv_timeout(Duration::from_secs(5))
        .expect("the client connected is told");
    assert_eq!(
        told,
        ServerMessage::ChatContinued {
            chat: CHAT,
            started: None,
            failure: Some("the agent crashed".to_owned()),
        }
    );
}

/// An agent that fails while writing the summary gives its own reason, not that the summary was stopped, and so does an account that needs signing in again.
#[test]
fn a_failed_summary_says_why() {
    let _forget = Forget;
    let (asker, _inbox) = client();
    let mut session = chat(&asker);
    session.take_in(CHAT, Event::Failed("the agent crashed".to_owned()));
    let outcome = session.handoff.and_then(|handoff| handoff.outcome);
    assert_eq!(outcome, Some(Err("the agent crashed".to_owned())));

    let mut session = chat(&asker);
    session.take_in(
        CHAT,
        Event::SignedOut {
            reason: "the login expired".to_owned(),
            account: None,
        },
    );
    let outcome = session.handoff.and_then(|handoff| handoff.outcome);
    assert_eq!(outcome, Some(Err("the login expired".to_owned())));
}

/// A chat that is closed while its agent writes the summary is not continued.
#[test]
fn closing_the_chat_gives_up_the_handoff() {
    let _forget = Forget;
    let (asker, _inbox) = client();
    let mut session = chat(&asker);

    session.fail_handoff("the chat was closed");
    session.settle_handoff();

    let outcome = session.handoff.and_then(|handoff| handoff.outcome);
    assert_eq!(outcome, Some(Err("the chat was closed".to_owned())));
}
