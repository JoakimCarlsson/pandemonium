//! Tests of how the server keeps the trust requests of its sessions open, answers them and drops them.

use std::path::PathBuf;

use pm_harness::{Input, Skipped, Standing};

use super::*;
use crate::sessions::Meta;

/// The numbers the sessions of these tests go by, which no real session has.
const NUMBERS: [u64; 3] = [4_000_000_301, 4_000_000_302, 4_000_000_303];

/// Removes what a test left of the sessions numbered in `NUMBERS` in the app's data folder when it goes out of scope, even when the test fails.
struct Forget;

impl Drop for Forget {
    /// Removes the folders of the sessions numbered in `NUMBERS`.
    fn drop(&mut self) {
        if let Some(root) = pm_store::data_directory() {
            for number in NUMBERS {
                let _ = std::fs::remove_dir_all(root.join("sessions").join(number.to_string()));
            }
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
    };
    Session::new(pm_harness::agents()[0], PathBuf::new(), log, meta)
}

/// A request under `ticket` for the repository at `root`, which left out its instructions.
fn request(ticket: u64, root: &str) -> Event {
    Event::Trust(TrustRequest {
        ticket,
        root: PathBuf::from(root),
        standing: Standing::Unknown,
        skipped: vec![Skipped {
            kind: Input::Instruction,
            name: format!("{root}/AGENTS.md"),
            path: PathBuf::from(format!("{root}/AGENTS.md")),
        }],
    })
}

/// The latest request is the open one, a `Trusted` under its ticket closes it, and one under an earlier ticket does not.
#[test]
fn the_latest_unanswered_request_is_open() {
    let held = session(vec![(1, request(1, "/r")), (2, request(2, "/r"))]);
    assert_eq!(held.open_trust().map(|open| open.ticket), Some(2));
    let held = session(vec![
        (1, request(1, "/r")),
        (2, request(2, "/r")),
        (
            3,
            Event::Trusted {
                ticket: 1,
                verdict: None,
            },
        ),
    ]);
    assert_eq!(held.open_trust().map(|open| open.ticket), Some(2));
    let held = session(vec![
        (1, request(2, "/r")),
        (
            2,
            Event::Trusted {
                ticket: 2,
                verdict: Some(Verdict::Allow),
            },
        ),
    ]);
    assert!(held.open_trust().is_none());
}

/// Dropping the open request notes a `Trusted` without a verdict after what clients heard, and leaves nothing open; with none open it notes nothing.
#[test]
fn a_request_whose_agent_stopped_is_dropped() {
    let _forget = Forget;
    let mut held = session(vec![(1, request(5, "/r"))]);
    let told = held.drop_trust(NUMBERS[0]);
    assert_eq!(told.len(), 1);
    assert_eq!(told[0].1, 2);
    assert_eq!(
        told[0].2,
        Event::Trusted {
            ticket: 5,
            verdict: None
        }
    );
    assert!(held.open_trust().is_none());
    assert!(held.drop_trust(NUMBERS[0]).is_empty());
}

/// Trusting a repository for good answers the open requests of the other sessions of that repository too, and leaves another repository's alone.
#[test]
fn a_recorded_verdict_answers_every_session_of_the_repository() {
    let _forget = Forget;
    let mut sessions = HashMap::from([
        (NUMBERS[0], session(vec![(1, request(3, "/r"))])),
        (NUMBERS[1], session(vec![(1, request(8, "/r"))])),
        (NUMBERS[2], session(vec![(1, request(4, "/other"))])),
    ]);
    let told = answer(&mut sessions, NUMBERS[0], 3, Verdict::Always).unwrap();
    let answered: Vec<(u64, Event)> = told
        .into_iter()
        .map(|(number, _, event)| (number, event))
        .collect();
    assert_eq!(
        answered,
        [
            (
                NUMBERS[0],
                Event::Trusted {
                    ticket: 3,
                    verdict: Some(Verdict::Always)
                }
            ),
            (
                NUMBERS[1],
                Event::Trusted {
                    ticket: 8,
                    verdict: Some(Verdict::Always)
                }
            ),
        ]
    );
    assert!(sessions[&NUMBERS[1]].open_trust().is_none());
    assert_eq!(
        sessions[&NUMBERS[2]].open_trust().map(|open| open.ticket),
        Some(4)
    );
}

/// Trusting a project for one conversation answers that session alone, and a ticket the session no longer waits on is refused.
#[test]
fn a_verdict_for_one_conversation_stays_in_it() {
    let _forget = Forget;
    let mut sessions = HashMap::from([
        (NUMBERS[0], session(vec![(1, request(3, "/r"))])),
        (NUMBERS[1], session(vec![(1, request(8, "/r"))])),
    ]);
    assert!(answer(&mut sessions, NUMBERS[0], 2, Verdict::Allow).is_err());
    let told = answer(&mut sessions, NUMBERS[0], 3, Verdict::Allow).unwrap();
    assert_eq!(told.len(), 1);
    assert_eq!(
        sessions[&NUMBERS[1]].open_trust().map(|open| open.ticket),
        Some(8)
    );
    assert!(answer(&mut sessions, NUMBERS[0], 3, Verdict::Allow).is_err());
}
