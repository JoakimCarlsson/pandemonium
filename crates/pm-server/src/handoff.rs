//! Continuing a chat as a session in a project: the chat's agent is asked for a handoff summary, and when it has written one a session starts in the project with the summary as its first prompt.
//!
//! The server runs the whole of it, so that it completes when the client that asked has gone.

use std::sync::Arc;

use pm_protocol::{Response, ServerMessage};

use crate::conversation;
use crate::core::{Connection, Core, Session};
use crate::handler::{failed, has_worktrees, place, start_session};
use crate::queue::Prompt;

/// What the chat's agent is asked, as an ordinary turn of the chat, so that the session that continues it needs nothing else of the conversation.
pub(crate) const PROMPT: &str = "Write a handoff summary of this conversation, so that the work can go on as a new session in a project. Write it for a reader who has seen none of this conversation. Cover, in this order: the goal; the decisions made and why; what was found; the files in the scratch folder worth reading, each with its absolute path; and the next steps. Reply with the summary only.";

/// A chat being continued in a project, from when its agent is asked for the summary until the session is started or given up.
pub(crate) struct Handoff {
    /// The folder of the project the session starts in.
    pub(crate) repository: String,
    /// The branch or commit the session's worktree is made from.
    pub(crate) base: String,
    /// What the session is called, which names its worktree.
    pub(crate) title: String,
    /// The client that asked, which is told how it went while it is still there.
    pub(crate) asker: Arc<Connection>,
    /// The summary the agent wrote, or why there is none, once the turn that asked for it is over.
    pub(crate) outcome: Option<Result<String, String>>,
}

impl Session {
    /// Notes how the turn that asked for the handoff summary ended, when this session is being continued in a project and that was not noted yet: with the agent's final message, or with why there is none.
    pub(crate) fn settle_handoff(&mut self) {
        if let Some(handoff) = &mut self.handoff
            && handoff.outcome.is_none()
        {
            handoff.outcome = Some(conversation::final_reply(&self.log));
        }
    }

    /// Gives up continuing this session in a project for `reason`, when it is being continued and how that went was not noted yet.
    pub(crate) fn fail_handoff(&mut self, reason: &str) {
        if let Some(handoff) = &mut self.handoff
            && handoff.outcome.is_none()
        {
            handoff.outcome = Some(Err(reason.to_owned()));
        }
    }
}

/// Asks the agent of the chat numbered `number` for a handoff summary, as a turn of the chat, and remembers to start a session called `title` in `repository`, in a worktree made from `base`, when it has written one; `asker` is told how that went.
///
/// Answers `Done` once the agent was asked, and fails when the session is no chat, when its agent is working or has prompts waiting, when it is being continued already, when its scratch folder is gone, or when `repository` holds no git repository to make a worktree of.
pub(crate) fn begin(
    core: &Core,
    asker: &Arc<Connection>,
    number: u64,
    repository: &str,
    base: &str,
    title: &str,
) -> Response {
    if let Err(reason) = has_worktrees(repository) {
        return failed(reason);
    }
    let told = {
        let Ok(mut sessions) = core.sessions.lock() else {
            return failed("the server is shutting down");
        };
        let Some(held) = sessions.get_mut(&number) else {
            return failed(format!("no session is numbered {number}"));
        };
        if !held.meta.chat {
            return failed(format!("{} is not a chat", held.meta.title));
        }
        if held.busy || !held.queue.is_empty() {
            return failed("the chat's agent is working");
        }
        if held.handoff.is_some() {
            return failed("the chat is being continued already");
        }
        if !place(&held.meta.directory).is_dir() {
            return failed(format!("the folder of {} was removed", held.meta.title));
        }
        held.handoff = Some(Handoff {
            repository: repository.to_owned(),
            base: base.to_owned(),
            title: title.to_owned(),
            asker: asker.clone(),
            outcome: None,
        });
        let asked = Prompt {
            text: PROMPT.to_owned(),
            paths: Vec::new(),
            names: Vec::new(),
        };
        held.submit(number, asked, false)
    };
    core.tell_all(told);
    Response::Done
}

/// A chat whose summary turn is over, with what starting the session that continues it needs of the chat.
struct Settled {
    /// The chat's number.
    chat: u64,
    /// What the chat is called.
    name: String,
    /// The chat's agent, by its id.
    agent: String,
    /// The settings the chat's agent last said it had, as it names them and their values.
    settings: Vec<(String, String)>,
    /// Where the session starts, who asked, and the summary or why there is none.
    handoff: Handoff,
}

/// Starts, each on a thread of its own, the session of every chat whose agent has written its handoff summary, and tells the client that asked, or every client when it has gone; a chat whose summary turn failed or was stopped gets no session, and the client is told why.
pub(crate) fn deliver(core: &Arc<Core>) {
    let settled: Vec<Settled> = {
        let Ok(mut sessions) = core.sessions.lock() else {
            return;
        };
        sessions
            .iter_mut()
            .filter(|(_, session)| {
                session
                    .handoff
                    .as_ref()
                    .is_some_and(|handoff| handoff.outcome.is_some())
            })
            .filter_map(|(number, session)| {
                Some(Settled {
                    chat: *number,
                    name: session.meta.title.clone(),
                    agent: session.meta.agent.clone(),
                    settings: conversation::settings(&session.log),
                    handoff: session.handoff.take()?,
                })
            })
            .collect()
    };
    for chat in settled {
        let core = core.clone();
        std::thread::spawn(move || finish(&core, chat));
    }
}

/// Starts the session that continues `settled` when its agent wrote a summary, with the summary as its first prompt under a line naming the chat, and tells the client that asked of the session, or why there is none; when that client has gone, every client connected is told instead, so that it hears of it after connecting again.
fn finish(core: &Arc<Core>, mut settled: Settled) {
    let asker = settled.handoff.asker.clone();
    let outcome = settled
        .handoff
        .outcome
        .take()
        .unwrap_or_else(|| Err("the summary did not finish".to_owned()))
        .and_then(|summary| {
            let text = format!("Continued from the chat \"{}\".\n\n{summary}", settled.name);
            let started = start_session(
                core,
                &asker,
                &settled.agent,
                &settled.handoff.repository,
                &settled.handoff.repository,
                &settled.handoff.title,
                Some(&settled.handoff.base),
                &settled.settings,
                &text,
                &[],
                None,
                false,
                false,
            );
            match started {
                Response::Started { session, .. } => core
                    .info(session)
                    .ok_or_else(|| format!("session {session} went away as it was made")),
                Response::Failed { message } => Err(message),
                _ => Err("the session could not be started".to_owned()),
            }
        });
    let (started, failure) = match outcome {
        Ok(info) => (Some(info), None),
        Err(reason) => (None, Some(reason)),
    };
    let told = ServerMessage::ChatContinued {
        chat: settled.chat,
        started,
        failure,
    };
    if asker.outbox.send(told.clone()).is_err() {
        core.broadcast(&told, None);
    }
}

#[cfg(test)]
mod tests;
