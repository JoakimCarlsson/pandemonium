//! Tests of how a session takes in a request to trust its project, and how the card answers it.

use std::path::PathBuf;

use pm_harness::Event;

use super::*;
use crate::sample::{Session, Status};

/// A fresh session whose server events can be replayed deterministically.
fn session() -> Session {
    Session::restored("Task", 0, "task", "/project", None, 0, false)
}

/// A request under `ticket` for the repository at `/r` that left out a hook, an instruction file and a skill.
fn request(ticket: u64) -> TrustRequest {
    TrustRequest {
        ticket,
        root: PathBuf::from("/r"),
        standing: Standing::Unknown,
        skipped: vec![
            Skipped {
                kind: Input::Hook,
                name: "PreToolUse bash: ./guard.sh".to_owned(),
                path: PathBuf::from("/r/.claude/settings.json"),
            },
            Skipped {
                kind: Input::Instruction,
                name: "/r/AGENTS.md".to_owned(),
                path: PathBuf::from("/r/AGENTS.md"),
            },
            Skipped {
                kind: Input::Skill,
                name: "release".to_owned(),
                path: PathBuf::from("/r/.claude/skills/release/SKILL.md"),
            },
        ],
    }
}

/// A request is held and asks for the reader's attention; a later one replaces it, and only an answer under the ticket held takes it away.
#[test]
fn a_request_waits_for_its_own_answer() {
    let mut held = session();
    held.hear(Event::Trust(request(1)));
    assert_eq!(held.shown(), Status::NeedsInput);
    held.hear(Event::Trust(request(2)));
    held.hear(Event::Trusted {
        ticket: 1,
        verdict: None,
    });
    assert_eq!(held.trust.as_ref().map(|open| open.ticket), Some(2));
    held.hear(Event::Trusted {
        ticket: 2,
        verdict: Some(Verdict::Always),
    });
    assert!(held.trust.is_none());
    assert_eq!(held.shown(), Status::Idle);
}

/// Each button sends its verdict for the open session's request under its ticket, and the card goes at once.
#[test]
fn each_button_answers_with_its_verdict() {
    for (button, verdict) in [
        (TRUST_ALWAYS, Verdict::Always),
        (TRUST_SESSION, Verdict::Allow),
        (TRUST_DECLINE, Verdict::Reject),
    ] {
        let mut state = WorkspaceState::sample();
        state.sessions[0] = session();
        state = state.heard(0, Event::Trust(request(7)));
        state.page = Page::Session(0);
        state.outbox.clear();
        state = state.after(button);
        assert!(state.sessions[0].trust.is_none());
        assert!(state.outbox.contains(&Outgoing::Trust {
            session: 0,
            ticket: 7,
            verdict,
        }));
    }
}

/// The card's words name the project, say what is not used in the order the card lists it, and say so differently when only its hooks changed.
#[test]
fn the_words_follow_the_standing() {
    let (title, said) = words(&request(1));
    assert_eq!(title, "Trust r?");
    assert!(
        said.contains("not using its hooks, instructions and skills"),
        "{said}"
    );
    let changed = TrustRequest {
        standing: Standing::Changed,
        ..request(1)
    };
    let (title, said) = words(&changed);
    assert_eq!(title, "The hooks of r changed since you trusted it");
    assert!(said.contains("still used"), "{said}");
    assert_eq!(
        from_root(Path::new("/r/AGENTS.md"), Path::new("/r")),
        "AGENTS.md"
    );
    assert_eq!(
        from_root(Path::new("/x/AGENTS.md"), Path::new("/r")),
        "/x/AGENTS.md"
    );
}
