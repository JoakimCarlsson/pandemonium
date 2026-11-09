//! Whether the agents may use what a project folder brings: the trust requests they make, and the reader's answers to them.
//!
//! An agent that left out a project's hooks, instruction files, skills or
//! agent definitions notes a `Trust` event in its session's log and waits for
//! an answer. The request stays open until a `Trusted` event under its ticket
//! closes it: the reader answered it, or the agent it came from stopped, so
//! nothing waits on it any more. Trust is recorded for a whole repository, so a
//! verdict that is recorded, trusting it for good or not at all, answers the
//! open requests of every other session of the same repository as well.

use std::collections::HashMap;

use pm_harness::{Event, TrustRequest, Verdict};

use crate::core::{Session, Told};

impl Session {
    /// The trust request the agent waits on an answer to: the latest one in the log that no `Trusted` event closed since.
    pub(crate) fn open_trust(&self) -> Option<&TrustRequest> {
        let mut closed = Vec::new();
        for (_, event) in self.log.iter().rev() {
            match event {
                Event::Trusted { ticket, .. } => closed.push(*ticket),
                Event::Trust(request) if !closed.contains(&request.ticket) => {
                    return Some(request);
                }
                Event::Trust(_) => return None,
                _ => {}
            }
        }
        None
    }

    /// Closes the open trust request of this session, numbered `number`, with `verdict`, or with none when it is dropped unanswered, and returns what it noted; nothing when no request is open.
    ///
    /// The agent is handed the verdict when there is one and it is running.
    pub(crate) fn settle_trust(&mut self, number: u64, verdict: Option<Verdict>) -> Told {
        let Some(ticket) = self.open_trust().map(|request| request.ticket) else {
            return Told::new();
        };
        if let (Some(verdict), Some(chat)) = (verdict, &self.chat) {
            chat.trust(ticket, verdict);
        }
        let event = Event::Trusted { ticket, verdict };
        vec![(number, self.note(number, &event), event)]
    }

    /// Drops the open trust request of this session, numbered `number`, as the agent it came from stopped and no longer waits on it, and returns what it noted.
    pub(crate) fn drop_trust(&mut self, number: u64) -> Told {
        self.settle_trust(number, None)
    }
}

/// The sessions among `sessions` whose open trust request a verdict recorded for the repository of the request of session `number` also answers: every other session waiting on a request for the same root, when `verdict` is recorded for the repository rather than kept to one conversation.
fn sharing(sessions: &HashMap<u64, Session>, number: u64, verdict: Verdict) -> Vec<u64> {
    let Some(root) = sessions
        .get(&number)
        .and_then(Session::open_trust)
        .map(|request| request.root.clone())
    else {
        return Vec::new();
    };
    if verdict == Verdict::Allow {
        return Vec::new();
    }
    let mut found: Vec<u64> = sessions
        .iter()
        .filter(|(other, session)| {
            **other != number
                && session
                    .open_trust()
                    .is_some_and(|request| request.root == root)
        })
        .map(|(other, _)| *other)
        .collect();
    found.sort_unstable();
    found
}

/// Answers the trust request under `ticket` of session `number` among `sessions` with `verdict`, and the open requests of the other sessions of the same repository when the verdict is recorded for it, and returns what it noted.
///
/// # Errors
///
/// Fails with the reason when there is no such session, or its agent waits on no request under `ticket`.
pub(crate) fn answer(
    sessions: &mut HashMap<u64, Session>,
    number: u64,
    ticket: u64,
    verdict: Verdict,
) -> Result<Told, String> {
    let session = sessions
        .get(&number)
        .ok_or_else(|| format!("no session is numbered {number}"))?;
    if session.open_trust().map(|request| request.ticket) != Some(ticket) {
        return Err(format!(
            "session {number} waits on no trust request under {ticket}"
        ));
    }
    let mut told = Told::new();
    for answered in std::iter::once(number).chain(sharing(sessions, number, verdict)) {
        if let Some(session) = sessions.get_mut(&answered) {
            told.extend(session.settle_trust(answered, Some(verdict)));
        }
    }
    Ok(told)
}

#[cfg(test)]
mod tests;
