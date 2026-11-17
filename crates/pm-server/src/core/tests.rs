//! Tests of how the server ends the turns its sessions leave open and numbers the turns it starts.

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
    Session::new(pm_harness::agents()[0], PathBuf::new(), log, meta)
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
                turn: 1,
                saved: None,
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

/// Ending the turn a stopped server left open does not change when the session was last worked in, once that is said again.
#[test]
fn an_interrupted_turn_keeps_when_the_session_was_last_worked_in() {
    let _forget = Forget;
    let mut held = session(vec![(
        1,
        Event::Prompted {
            text: "work".to_owned(),
            files: Vec::new(),
            checkpoint: false,
            turn: 1,
            saved: None,
        },
    )]);
    held.meta.updated = 7;

    held.agent_gone(NUMBER, INTERRUPTED);
    assert_ne!(held.meta.updated, 7);
    held.keep_updated(NUMBER, 7);

    assert_eq!(held.meta.updated, 7);
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
            turn: 1,
            saved: None,
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

/// The command `id`, started by the call `call-<id>`, starting or ending as `running` says.
fn command(id: &str, running: bool) -> Event {
    Event::Background(pm_harness::Background {
        id: id.to_owned(),
        label: "cargo watch".to_owned(),
        running,
        call: Some(format!("call-{id}")),
    })
}

/// A log of a turn left open with the command `s1` still running and the command `s2` that ended.
fn left_running() -> Vec<(u64, Event)> {
    vec![
        (
            1,
            Event::Prompted {
                text: "work".to_owned(),
                files: Vec::new(),
                checkpoint: false,
                turn: 1,
                saved: None,
            },
        ),
        (2, command("s1", true)),
        (3, command("s2", true)),
        (4, command("s2", false)),
    ]
}

/// An agent found gone, as when the server starts again, ends the commands its log leaves running, with the calls that started them, before the turn it left open, and only once.
#[test]
fn a_gone_agent_ends_the_commands_it_left_running() {
    let _forget = Forget;
    let mut held = session(left_running());

    let told = held.agent_gone(NUMBER, INTERRUPTED);

    let seqs: Vec<u64> = told.iter().map(|(_, seq, _)| *seq).collect();
    assert_eq!(seqs, [5, 6, 7]);
    assert_eq!(told[0].2, command("s1", false));
    assert!(matches!(told[1].2, Event::Halted));
    assert!(matches!(&told[2].2, Event::Finished(Some(reason)) if reason == INTERRUPTED));
    assert!(held.agent_gone(NUMBER, INTERRUPTED).is_empty());
}

/// An agent that fails ends the commands it left running too.
#[test]
fn a_failed_agent_ends_the_commands_it_left_running() {
    let _forget = Forget;
    let mut held = session(left_running());

    let told = held.take_in(NUMBER, Event::Failed("the agent stopped".to_owned()));

    let ended: Vec<&Event> = told
        .iter()
        .map(|(_, _, event)| event)
        .filter(|event| matches!(event, Event::Background(_)))
        .collect();
    assert_eq!(ended, [&command("s1", false)]);
    assert!(held.end_background(NUMBER).is_empty());
}

/// A turn id never repeats one the log holds, even when the log was rebuilt with fewer events than its ids reach.
#[test]
fn next_turn_id_stays_above_every_id_in_the_log() {
    let prompted = |turn: u64| Event::Prompted {
        text: String::new(),
        files: Vec::new(),
        checkpoint: false,
        turn,
        saved: None,
    };
    let rebuilt = session(vec![
        (1, prompted(2)),
        (2, prompted(40)),
        (3, Event::Finished(None)),
    ]);
    assert_eq!(rebuilt.next_turn_id(), 41);
    assert_eq!(session(Vec::new()).next_turn_id(), 1);
}

/// What the agent answers about the files of a turn is noted under the reader's turn it names, by its place in the conversation, and goes to the request that asked first; an answer about a turn the conversation no longer holds, and a failure, are noted nowhere.
#[test]
fn file_answers_are_noted_by_turn_and_handed_to_who_asked() {
    use pm_harness::FileNews;
    let _forget = Forget;
    let prompted = |turn: u64| Event::Prompted {
        text: String::new(),
        files: Vec::new(),
        checkpoint: true,
        turn,
        saved: None,
    };
    let mut held = session(vec![
        (1, prompted(4)),
        (2, Event::Finished(None)),
        (3, prompted(9)),
        (4, Event::Finished(None)),
    ]);
    let (first, answered_first) = std::sync::mpsc::channel();
    let (second, answered_second) = std::sync::mpsc::channel();
    held.file_waiters.push_back(first);
    held.file_waiters.push_back(second);

    let told = held.take_files(
        NUMBER,
        FileNews::Undone {
            turn: "9".to_owned(),
            files: vec!["a.rs".to_owned()],
        },
    );
    assert!(matches!(&told[0].2, Event::Undone { turn: 1, files } if files == &["a.rs"]));
    assert!(matches!(
        answered_first.try_recv(),
        Ok(FileNews::Undone { .. })
    ));
    assert!(answered_second.try_recv().is_err());

    let told = held.take_files(NUMBER, FileNews::Failed("no files were kept".to_owned()));
    assert!(told.is_empty());
    assert!(matches!(
        answered_second.try_recv(),
        Ok(FileNews::Failed(_))
    ));

    let gone = FileNews::Restored {
        turn: "77".to_owned(),
        files: vec!["a.rs".to_owned()],
    };
    assert!(held.take_files(NUMBER, gone).is_empty());
    let told = held.take_files(
        NUMBER,
        FileNews::Restored {
            turn: "4".to_owned(),
            files: Vec::new(),
        },
    );
    assert!(matches!(&told[0].2, Event::Restored { turn: 0, .. }));
    assert!(held.ask_changes(NUMBER, 5).is_err());

    held.take_in(
        NUMBER,
        Event::Prompted {
            text: "toward the goal".to_owned(),
            files: Vec::new(),
            checkpoint: true,
            turn: 0,
            saved: Some("goal-12".to_owned()),
        },
    );
    held.take_in(NUMBER, Event::Finished(None));
    let told = held.take_files(
        NUMBER,
        FileNews::Restored {
            turn: "goal-12".to_owned(),
            files: Vec::new(),
        },
    );
    assert!(matches!(&told[0].2, Event::Restored { turn: 2, .. }));
    let turns = crate::conversation::turns(&held.log);
    assert_eq!(turns[2].saved, "goal-12");
    assert_eq!(turns[1].saved, "9");
}
