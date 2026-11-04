//! Tests of editing the prompts that wait for the agent's turn to end.

use super::*;
use crate::sessions::Meta;

/// The number the sessions of these tests go by, which no real session has.
const NUMBER: u64 = 4_000_000_101;

/// A session of the first agent with no agent running and nothing in its log.
fn session() -> Session {
    let meta = Meta {
        agent: pm_acp::AGENTS[0].id.to_owned(),
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
    };
    Session::new(
        pm_acp::AGENTS[0],
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

    let told = held.edit_queued(NUMBER, first, "first, edited".to_owned());

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

    assert!(held.edit_queued(NUMBER, id, "late".to_owned()).is_empty());
    assert!(
        held.edit_queued(NUMBER, id + 9, "never".to_owned())
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

    assert!(held.edit_queued(NUMBER, id, "  ".to_owned()).is_empty());
    assert_eq!(held.queue[0].prompt.text, "keep");
}
