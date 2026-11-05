//! Tests of how the server ends the turns its sessions leave open.

use super::*;

/// The number the sessions of these tests go by, which no real session has.
const NUMBER: u64 = 4_000_000_201;

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

/// A session of the first agent with no agent running, holding `log`.
fn session(log: Vec<(u64, Event)>) -> Session {
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
    Session::new(pm_acp::AGENTS[0], PathBuf::new(), log, meta)
}

/// A turn left open when the server stopped ends with events numbered after everything a client already heard, so a client attaching again from its last event hears it end.
#[test]
fn interrupted_turn_ends_after_what_clients_heard() {
    let _forget = Forget;
    let mut held = session(vec![
        (
            1,
            Event::Prompted {
                text: "work".to_owned(),
                files: Vec::new(),
                checkpoint: false,
            },
        ),
        (2, Event::Said("partly done".to_owned())),
    ]);
    assert!(held.turn_open());

    let told = held.end_open_turn(NUMBER, INTERRUPTED);

    let seqs: Vec<u64> = told.iter().map(|(_, seq, _)| *seq).collect();
    assert_eq!(seqs, [3, 4]);
    assert!(matches!(told[0].2, Event::Halted));
    assert!(matches!(&told[1].2, Event::Finished(Some(reason)) if reason == INTERRUPTED));
    assert!(!held.turn_open());
    assert!(held.end_open_turn(NUMBER, INTERRUPTED).is_empty());
}

/// An agent that stops in the middle of a turn ends the turn with an error, after noting that what it had running stopped with it.
#[test]
fn agent_stopping_mid_turn_ends_the_turn_with_an_error() {
    let _forget = Forget;
    let mut held = session(vec![(
        1,
        Event::Prompted {
            text: "work".to_owned(),
            files: Vec::new(),
            checkpoint: false,
        },
    )]);
    held.busy = true;

    let told = held.take_in(NUMBER, Event::Failed("the agent stopped".to_owned()));

    let seqs: Vec<u64> = told.iter().map(|(_, seq, _)| *seq).collect();
    assert_eq!(seqs, [2, 3]);
    assert!(matches!(told[0].2, Event::Halted));
    assert!(matches!(&told[1].2, Event::Failed(reason) if reason == "the agent stopped"));
    assert!(!held.turn_open());
    assert!(!held.busy);
    assert!(held.chat.is_none());
}
