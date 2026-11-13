//! Tests of editing the prompts that wait for the agent's turn to end.

use super::*;
use crate::sessions::Meta;

/// The number the sessions of these tests go by, which no real session has.
const NUMBER: u64 = 4_000_000_101;

/// The client the tests act as.
const HOLDER: usize = 1;

/// A session of the first agent with no agent running and nothing in its log.
fn session() -> Session {
    let meta = Meta {
        agent: pm_harness::agents()[0].id.to_owned(),
        repository: String::new(),
        directory: String::new(),
        branch: None,
        head: None,
        base: None,
        title: "test".to_owned(),
        closed: false,
        updated: 0,
        login: None,
        submodule: None,
        container: false,
        chat: false,
        started_by: None,
    };
    Session::new(
        pm_harness::agents()[0],
        std::path::PathBuf::new(),
        Vec::new(),
        meta,
    )
}

/// A prompt saying `text` with no files.
fn prompt(text: &str) -> Prompt {
    Prompt {
        text: text.to_owned(),
        paths: Vec::new(),
        names: vec!["notes.txt".to_owned()],
    }
}

/// Removes what a test left of the session numbered `NUMBER` in the app's data folder when it goes out of scope, even when the test fails.
struct Forget;

impl Drop for Forget {
    /// Removes the folder of the session numbered `NUMBER`.
    fn drop(&mut self) {
        if let Some(root) = pm_store::data_directory() {
            let _ = std::fs::remove_dir_all(root.join("sessions").join(NUMBER.to_string()));
        }
    }
}

/// Editing a waiting prompt replaces its text, keeps its place, id and files, and announces it again under the same id.
#[test]
fn editing_a_waiting_prompt_keeps_its_place_and_files() {
    let _forget = Forget;
    let mut held = session();
    held.busy = true;
    held.submit(NUMBER, prompt("first"), false);
    held.submit(NUMBER, prompt("second"), false);
    let first = held.queue[0].id;

    let told = held.edit_queued(NUMBER, HOLDER, first, "first, edited".to_owned());

    assert_eq!(told.len(), 1);
    assert!(matches!(
        &told[0].2,
        Event::Queued { id, text, files }
            if *id == first && text == "first, edited" && files == &["notes.txt".to_owned()]
    ));
    let texts: Vec<&str> = held
        .queue
        .iter()
        .map(|waiting| waiting.prompt.text.as_str())
        .collect();
    assert_eq!(texts, ["first, edited", "second"]);
    assert_eq!(held.queue[0].id, first);
    assert_eq!(held.queue[0].prompt.names, ["notes.txt"]);
}

/// Editing a prompt the agent already took, or one that never was, changes nothing and announces nothing.
#[test]
fn editing_a_prompt_that_left_the_queue_is_ignored() {
    let _forget = Forget;
    let mut held = session();
    held.busy = true;
    held.submit(NUMBER, prompt("only"), false);
    let id = held.queue[0].id;
    held.queue.pop_front();
    let logged = held.log.len();

    assert!(
        held.edit_queued(NUMBER, HOLDER, id, "late".to_owned())
            .is_empty()
    );
    assert!(
        held.edit_queued(NUMBER, HOLDER, id + 9, "never".to_owned())
            .is_empty()
    );
    assert_eq!(held.log.len(), logged);
    assert!(held.queue.is_empty());
}

/// An edit that leaves a prompt with neither text nor files is ignored.
#[test]
fn editing_a_prompt_to_nothing_is_ignored() {
    let _forget = Forget;
    let mut held = session();
    held.busy = true;
    held.submit(NUMBER, prompt("keep"), false);
    let id = held.queue[0].id;
    held.queue[0].prompt.paths.clear();

    assert!(
        held.edit_queued(NUMBER, HOLDER, id, "  ".to_owned())
            .is_empty()
    );
    assert_eq!(held.queue[0].prompt.text, "keep");
}

/// A queue a client holds hands nothing over while an edit is open, and lets go of the prompts when the holder ends its edit; another client can neither hold nor release it.
#[test]
fn a_held_queue_hands_nothing_over_until_its_holder_lets_go() {
    let _forget = Forget;
    let mut held = session();
    held.busy = true;
    held.submit(NUMBER, prompt("first"), false);
    held.hold_queue(NUMBER, HOLDER, true);
    held.hold_queue(NUMBER, HOLDER + 1, true);
    held.busy = false;

    assert!(held.advance(NUMBER).is_empty());
    assert_eq!(held.queue.len(), 1);
    assert!(held.hold_queue(NUMBER, HOLDER + 1, false).is_empty());
    assert_eq!(held.held_by, Some(HOLDER));

    held.edit_queued(NUMBER, HOLDER + 1, held.queue[0].id, "other".to_owned());
    assert_eq!(held.held_by, Some(HOLDER));

    held.busy = true;
    let id = held.queue[0].id;
    held.edit_queued(NUMBER, HOLDER, id, "mine".to_owned());
    assert_eq!(held.held_by, None);
    assert_eq!(held.queue[0].prompt.text, "mine");
}

/// Showing, pausing, clearing and budgeting the goal are handed to a working agent at once; setting or resuming it, and other prompts, wait.
#[test]
fn goal_commands_that_start_no_turn_steer_at_once() {
    for text in ["/goal", " /goal pause", "/goal clear", "/goal budget 2m"] {
        assert!(steers_goal(text), "{text}");
    }
    for text in [
        "/goal resume",
        "/goal ship it",
        "/goal pause the build",
        "fix it",
    ] {
        assert!(!steers_goal(text), "{text}");
    }
}

/// A prompt the agent reports, a turn it started itself toward its goal, makes the session busy, so that prompts sent meanwhile wait for it.
#[test]
fn a_turn_the_agent_starts_makes_the_session_busy() {
    let _forget = Forget;
    let mut held = session();
    held.take_in(
        NUMBER,
        Event::Prompted {
            text: "Keep working toward the goal".into(),
            files: Vec::new(),
            checkpoint: false,
            turn: 0,
        },
    );
    assert!(held.busy);
    held.take_in(NUMBER, Event::Finished(None));
    assert!(!held.busy);
}
