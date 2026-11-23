//! Tests of the steps a recording kept and how the reader steps through them.

use super::{AgentStep, Steps};

/// Step `index` of the session `session`, with no pictures.
fn step_of(session: &str, index: u32) -> AgentStep {
    AgentStep {
        session: session.to_owned(),
        session_name: format!("Task {session}"),
        ..step(index)
    }
}

/// A step numbered `index`, with no pictures.
fn step(index: u32) -> AgentStep {
    AgentStep {
        index,
        tool: "browser_click".to_owned(),
        arguments: "{\"ref\":3}".to_owned(),
        ok: true,
        text: "Clicked.".to_owned(),
        url: "http://localhost:5173/".to_owned(),
        session: "session-1".to_owned(),
        session_name: "Sign in".to_owned(),
        thumbnail: None,
        picture: None,
    }
}

/// The first step of a recording opens the panel, which follows the newest step until one is chosen.
#[test]
fn a_recording_opens_the_panel_and_follows_the_newest() {
    let mut steps = Steps::default();
    assert_eq!(steps.current(), None);
    steps.add("/r/one", step(1));
    steps.add("/r/one", step(2));
    assert!(steps.open);
    assert_eq!(steps.current(), Some(1));
    assert_eq!(steps.list[0].short_tool(), "click");
}

/// Stepping stays within the list, and a new recording starts it again.
#[test]
fn stepping_stays_within_the_list_and_a_new_recording_starts_over() {
    let mut steps = Steps::default();
    for index in 1..=3 {
        steps.add("/r/one", step(index));
    }
    steps.step_by(-1);
    assert_eq!(steps.current(), Some(1));
    steps.step_by(-5);
    assert_eq!(steps.current(), Some(0));
    steps.step_by(9);
    assert_eq!(steps.current(), Some(2));
    steps.open = false;
    steps.add("/r/two", step(1));
    assert_eq!(steps.list.len(), 1);
    assert_eq!(steps.shown, None);
    assert!(steps.open);
}

/// Each session keeps its own latest recording, a new recording of one session leaves the others' steps, and the panel lists one session's steps when the reader chooses it.
#[test]
fn sessions_keep_their_own_recordings_and_can_be_listed_alone() {
    let mut steps = Steps::default();
    steps.add("/r/a", step_of("session-1", 1));
    steps.add("/r/b", step_of("session-2", 1));
    steps.add("/r/a", step_of("session-1", 2));
    steps.add("/r/b", step_of("session-2", 2));
    assert_eq!(steps.list.len(), 4);
    assert_eq!(steps.sessions().len(), 2);
    steps.choose_session(Some(0));
    assert_eq!(steps.visible(), vec![0, 2]);
    assert_eq!(steps.current(), Some(2));
    steps.step_by(-1);
    assert_eq!(steps.current(), Some(0));
    steps.step_by(-1);
    assert_eq!(steps.current(), Some(0));
    steps.add("/r/a2", step_of("session-1", 1));
    assert_eq!(steps.list.len(), 3);
    assert!(
        steps
            .list
            .iter()
            .filter(|held| held.session == "session-2")
            .count()
            == 2
    );
    assert_eq!(steps.visible().len(), 1);
    steps.choose_session(None);
    assert_eq!(steps.visible().len(), 3);
}
