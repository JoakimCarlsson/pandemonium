//! Tests of the sessions the quick input lists: those waiting for the reader first, the chats the servers found by what was said in them, and starting a session from what is typed.

use pm_protocol::{ChatField, ChatHit};
use pm_text::Edit;

use crate::listing::{Destination, Items};
use crate::state::{Outgoing, Page, WorkspaceState};

use super::waits_for_reader;

/// The sample workspace with the quick input listing sessions and `typed` typed in it.
fn typed(typed: &str) -> WorkspaceState {
    let state = WorkspaceState::sample().sessions_requested((0.0, 0.0));
    if typed.is_empty() {
        return state;
    }
    state.listing_filtered(Edit::Paste(typed.to_owned()))
}

/// The places the quick input lists, in the order it lists them.
fn listed(state: &WorkspaceState) -> Vec<Destination> {
    let Some(listing) = &state.listing else {
        return Vec::new();
    };
    let Items::Sessions { sessions, .. } = &listing.items else {
        return Vec::new();
    };
    listing
        .items
        .rows()
        .into_iter()
        .map(|row| sessions[row].0)
        .collect()
}

/// A hit for the session at `position` found in `field`, previewed as `preview`.
fn hit(position: usize, field: ChatField, preview: &str) -> (usize, ChatHit) {
    (
        position,
        ChatHit {
            session: position as u64,
            field,
            preview: preview.to_owned(),
            score: 1,
        },
    )
}

/// With nothing typed, the sessions waiting for the reader are listed before the rest.
#[test]
fn waiting_sessions_lead_while_nothing_is_typed() {
    let mut state = WorkspaceState::sample();
    state.page = Page::History;
    let last = state
        .sessions
        .iter()
        .rposition(|session| !session.home && !session.closed)
        .unwrap_or_default();
    for session in &mut state.sessions {
        session.read_through = session.updates;
        session.marked_unread = false;
        session.ask = None;
        session.inquiry = None;
        session.trust = None;
        session.signed_out = None;
        session.latest_error = false;
        session.status = crate::sample::Status::Idle;
        session.shells.clear();
    }
    state.sessions[last].marked_unread = true;
    state.sessions[last].updated_at = 0;
    let state = state.sessions_requested((0.0, 0.0));
    let rows = listed(&state);
    assert_eq!(rows.first(), Some(&Destination::Session(last)));
    assert!(waits_for_reader(&state.sessions[last]));
    assert!(
        state
            .outbox
            .iter()
            .all(|sent| !matches!(sent, Outgoing::SearchChats { .. }))
    );
}

/// Typing asks the servers for the chats whose conversations hold what is typed.
#[test]
fn typing_searches_the_chats() {
    let state = typed("parser");
    assert!(
        state
            .outbox
            .iter()
            .any(|sent| matches!(sent, Outgoing::SearchChats { query } if query == "parser"))
    );
}

/// Chats found by what was said in them follow the sessions matched by name, before the projects, each with a preview of the message; an answer for what is no longer typed is left out.
#[test]
fn found_chats_are_merged_with_previews() {
    let mut state = typed("zzqx");
    let closed = 1;
    state.sessions[closed].closed = true;
    let before = listed(&state);
    assert!(
        before
            .iter()
            .all(|row| matches!(row, Destination::Project(_)))
    );

    let stale = state
        .clone()
        .with_chats("other", vec![hit(0, ChatField::Reply, "nothing")]);
    assert_eq!(listed(&stale), before);

    let state = state.with_chats(
        "zzqx",
        vec![
            hit(0, ChatField::Prompt, "…the zzqx parser…"),
            hit(closed, ChatField::Reply, "zzqx again"),
        ],
    );
    let rows = listed(&state);
    assert_eq!(rows[0], Destination::Session(0));
    assert_eq!(rows[1], Destination::Session(closed));
    assert!(
        rows[2..]
            .iter()
            .all(|row| matches!(row, Destination::Project(_)))
    );
    let Some(Items::Sessions {
        sessions, found, ..
    }) = state.listing.as_ref().map(|listing| &listing.items)
    else {
        panic!("the quick input lists sessions");
    };
    let previews: Vec<&str> = found
        .iter()
        .map(|(place, preview)| {
            assert!(matches!(sessions[*place].0, Destination::Session(_)));
            preview.as_str()
        })
        .collect();
    assert_eq!(previews, vec!["…the zzqx parser…", "zzqx again"]);
}

/// A hit on a title adds the session but no preview, and a session is listed once however many servers name it.
#[test]
fn title_hits_have_no_preview_and_are_listed_once() {
    let state = typed("zzqx").with_chats(
        "zzqx",
        vec![
            hit(0, ChatField::Title, "zzqx"),
            hit(0, ChatField::Reply, "zzqx said"),
        ],
    );
    let rows = listed(&state);
    assert_eq!(
        rows.iter()
            .filter(|row| **row == Destination::Session(0))
            .count(),
        1
    );
    let Some(Items::Sessions { found, .. }) = state.listing.as_ref().map(|listing| &listing.items)
    else {
        panic!("the quick input lists sessions");
    };
    assert_eq!(found.len(), 1);
}

/// Control or Command with Enter turns what is typed into the prompt of a new session and closes the list.
#[test]
fn a_hurried_enter_starts_a_session_with_what_is_typed() {
    let state = typed("Write the release notes").listing_prompted();
    assert!(state.listing.is_none());
    assert!(matches!(state.page, Page::New(_)));
    assert_eq!(state.prompt.text(), "Write the release notes");
}
