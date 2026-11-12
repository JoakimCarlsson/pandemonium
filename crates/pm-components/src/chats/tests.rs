//! Tests of chats: starting one without a project, where it lives, and how it is kept apart from the projects.

use super::NEW_CHAT;
use crate::state::{Outgoing, Page, SEND, WorkspaceState, prompt_field};

/// Sends `text` from the page being shown and returns the state afterwards.
fn sent(mut state: WorkspaceState, text: &str) -> WorkspaceState {
    state.prompt = prompt_field(text);
    state.after(SEND)
}

/// What the server lists for a chat numbered `session`, working in the scratch folder `directory`.
fn listed(session: u64, directory: &str) -> pm_protocol::SessionInfo {
    pm_protocol::SessionInfo {
        session,
        agent: pm_harness::agents()[0].id.to_owned(),
        repository: String::new(),
        directory: directory.to_owned(),
        branch: None,
        head: None,
        base: None,
        title: "Chat".to_owned(),
        updated: 0,
        last_seq: 0,
        closed: false,
        running: false,
        login: None,
        submodule: None,
        chat: true,
    }
}

/// A window with no project opens on the page that starts a chat, and sending a prompt there starts a chat on this machine that belongs to no project.
#[test]
fn the_empty_window_starts_a_chat() {
    let state = WorkspaceState::new(&[]);
    assert_eq!(state.page, Page::Chat);
    assert!(state.starting());

    let mut state = sent(state, "What is a monad?");

    let Page::Session(started) = state.page else {
        panic!("the page did not move to the chat");
    };
    let chat = &state.sessions[started];
    assert!(chat.is_chat());
    assert_eq!(chat.machine, None);
    assert!(chat.branch.is_empty() && chat.base.is_none());
    assert_eq!(state.open_chats(), [started]);
    assert!(state.take_outbox().iter().any(|outgoing| matches!(
        outgoing,
        Outgoing::Start { session, base: None, container: false, .. } if *session == started
    )));
}

/// New chat goes to the machine of the project shown, and to this machine when no project is shown.
#[test]
fn a_chat_goes_to_the_machine_of_the_project_shown() {
    let mut state = WorkspaceState::new(&[])
        .with_project("/work/app")
        .with_project("ssh://builder/work/lib");
    state.start_in(1, None);
    let state = sent(state.after(NEW_CHAT), "Why is the build slow?");
    let Page::Session(started) = state.page else {
        panic!("the page did not move to the chat");
    };
    assert_eq!(state.sessions[started].machine.as_deref(), Some("builder"));
    assert!(state.is_on(&state.sessions[started], &Some("builder".to_owned())));

    let mut state = state;
    state.page = Page::History;
    let state = state.after(NEW_CHAT);
    assert_eq!(state.page, Page::Chat);
    assert_eq!(state.chat_on, None);
}

/// A chat the server lists is shown without a project, and stays when every project is taken away.
#[test]
fn a_listed_chat_outlives_the_projects() {
    let mut state = WorkspaceState::new(&[]).with_project("/work/app");
    let added = state.restore_sessions(&None, &[listed(7, "/data/chats/7")]);
    assert_eq!(added.len(), 1);
    let position = added[0].0;
    assert!(state.sessions[position].is_chat());
    assert_eq!(state.sessions[position].directory, "/data/chats/7");

    let state = state.without_project(0);

    assert!(state.projects.is_empty());
    assert_eq!(state.open_chats().len(), 1);
    assert_eq!(state.page, Page::Chat);
}

/// Closing the chat on screen archives it: it leaves the sidebar and the page that starts a chat on its machine shows.
#[test]
fn closing_a_chat_shows_the_page_that_starts_one() {
    let mut state = WorkspaceState::new(&[]).with_project("/work/app");
    let host = Some("builder".to_owned());
    let position = state.restore_sessions(&host, &[listed(7, "/data/chats/7")])[0].0;
    state.page = Page::Session(position);

    let state = state.closed_session(position, false, false);

    assert_eq!(state.page, Page::Chat);
    assert_eq!(state.chat_on, host);
    assert!(state.open_chats().is_empty());
}
