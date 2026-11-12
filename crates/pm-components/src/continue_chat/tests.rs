//! Tests of continuing a chat in a project: which projects are offered, what the choice asks of the server, and what the window does with the outcome.

use pm_input::FocusId;

use super::{CONTINUE, CONTINUE_PROJECT_BASE, CONTINUE_START};
use crate::sample::{Session, Status};
use crate::state::{BRANCH_BASE, Outgoing, Page, Picker, WorkspaceState};

/// A workspace of a project on this machine, one on the machine `builder` and a second on this machine, open on an idle chat on this machine, and the chat's position.
fn on_a_chat() -> (WorkspaceState, usize) {
    let mut state = WorkspaceState::new(&[])
        .with_project("/work/app")
        .with_project("ssh://builder/work/lib")
        .with_project("/work/site");
    state.sessions.push(Session::restored_chat(
        "Pick a parser",
        None,
        "/data/chats/7",
        0,
        false,
    ));
    let chat = state.sessions.len() - 1;
    state.page = Page::Session(chat);
    state.take_outbox();
    (state, chat)
}

/// Only the projects on the chat's own machine are offered.
#[test]
fn projects_on_other_machines_are_not_offered() {
    let (state, chat) = on_a_chat();
    assert_eq!(state.continue_projects(&state.sessions[chat]), [0, 2]);
}

/// The choice opens on the first project offered and reads its branches; choosing another project and a branch, then Continue, asks the server and switches the chip off until it answers.
#[test]
fn the_choice_asks_the_server_to_continue() {
    let (state, chat) = on_a_chat();

    let mut state = state.after(CONTINUE);
    assert_eq!(state.picker, Some(Picker::Continue));
    assert_eq!(state.continue_project(), Some(0));
    assert!(state.take_outbox().iter().any(
        |outgoing| matches!(outgoing, Outgoing::Branches { project: 0, host: None, directory } if directory == "/work/app")
    ));

    let state = state.after(FocusId(CONTINUE_PROJECT_BASE + 1));
    assert_eq!(
        state.continue_project(),
        Some(0),
        "another machine's project is not taken"
    );
    let mut state = state.after(FocusId(CONTINUE_PROJECT_BASE + 2));
    assert_eq!(state.continue_project(), Some(2));
    state.take_outbox();
    let state = state.with_branches(2, vec!["main".to_owned(), "next".to_owned()]);

    let state = state.after(FocusId(BRANCH_BASE + 1));
    assert_eq!(
        state.picker,
        Some(Picker::Continue),
        "choosing a branch keeps the choice open"
    );
    assert_eq!(
        (state.base.as_ref(), state.branches_of),
        (None, None),
        "the page for starting work keeps its own choice"
    );

    let mut state = state.after(CONTINUE_START);
    assert_eq!(state.picker, None);
    assert!(state.sessions[chat].handing_over);
    assert!(!state.can_continue(&state.sessions[chat]));
    assert!(state.take_outbox().iter().any(|outgoing| matches!(
        outgoing,
        Outgoing::ContinueChat { session, project: 2, base } if *session == chat && base == "next"
    )));
}

/// The choice does not open while the chat's agent is working or prompts wait for it.
#[test]
fn a_working_chat_cannot_be_continued() {
    let (mut state, chat) = on_a_chat();
    state.sessions[chat].status = Status::Working;
    assert!(!state.can_continue(&state.sessions[chat]));
    let mut state = state.after(CONTINUE);
    assert_eq!(state.picker, None);

    state.sessions[chat].status = Status::Idle;
    state.sessions[chat].queued.push(crate::sample::Queued {
        id: 1,
        text: "And then?".to_owned(),
        files: Vec::new(),
    });
    assert!(!state.can_continue(&state.sessions[chat]));
    assert_eq!(state.after(CONTINUE).picker, None);
}

/// When the server started the session the window moves to it and the chat stays listed; when it did not, the chat says why and can be continued again.
#[test]
fn the_window_follows_the_outcome() {
    let (mut state, chat) = on_a_chat();
    state.sessions[chat].handing_over = true;
    state.sessions.push(Session::restored(
        "Pick a parser",
        0,
        "main",
        "/work/app-wt",
        None,
        0,
        false,
    ));
    let started = state.sessions.len() - 1;

    let moved = state.clone().chat_continued(chat, Some(started), None);
    assert_eq!(moved.page, Page::Session(started));
    assert!(!moved.sessions[chat].handing_over);
    assert_eq!(moved.open_chats(), [chat]);

    let stayed = state.chat_continued(chat, None, Some("the summary was stopped"));
    assert_eq!(stayed.page, Page::Session(chat));
    assert!(stayed.can_continue(&stayed.sessions[chat]));
    let said = &stayed.sessions[chat].messages.last().expect("a note").text;
    assert!(said.contains("the summary was stopped"), "{said}");
}
