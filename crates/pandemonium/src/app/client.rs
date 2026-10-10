//! What the window does for an agent: its files and its terminals.
//!
//! The protocol makes the editor the agent's client, and the answers are the
//! window's because the things asked about are the reader's. A file an agent
//! reads is the file as the reader has it open, unsaved edits and all, and a
//! file it writes lands in that buffer as an edit the reader can take back. A
//! command it runs is a shell in the worktree's list, beside the reader's
//! own, which the reader can open, watch and stop.

use std::collections::BTreeMap;
use std::path::Path;

use pm_acp::{Answer, Exit, Request, Run};
use pm_core::Scope;

use crate::agent::TalkId;
use crate::app::App;
use crate::terminal::{Shell, ShellId};

/// The terminals agents have started, and the waits on them not yet over.
#[derive(Default)]
pub(super) struct Errands {
    /// Each terminal, by the conversation that started it and the name the
    /// agent knows it by, as the worktree and the shell it runs as.
    shells: BTreeMap<(TalkId, String), (Scope, ShellId)>,
    /// The requests to be answered once a terminal's command has exited, as
    /// the conversation, the ticket and the terminal's name.
    waits: Vec<(TalkId, u64, String)>,
}

impl App {
    /// Carries out every file and terminal request the agents have raised.
    pub(super) fn serve_agents(&mut self) {
        for (talk, ticket, request) in self.agents.take_requests() {
            let Some(scope) = self.agents.get(talk).map(|talk| talk.scope()) else {
                continue;
            };
            let answer = match request {
                Request::Read { path } => match self.read_for_agent(scope, &path) {
                    Some(answer) => answer,
                    None => {
                        self.answer_later(talk, ticket, move || read_from_disk(&path));
                        continue;
                    }
                },
                Request::Write { path, text } => match self.write_for_agent(scope, &path, &text) {
                    Some(answer) => answer,
                    None => {
                        self.answer_later(talk, ticket, move || write_to_disk(&path, &text));
                        continue;
                    }
                },
                Request::Run(run) => self.run_for_agent(talk, scope, run),
                Request::Output { terminal } => {
                    self.errand(talk, &terminal).map_or_else(gone, |shell| {
                        let mut shell = shell.borrow_mut();
                        Answer::Output {
                            text: shell.text(),
                            exit: exit(&mut shell),
                        }
                    })
                }
                Request::Wait { terminal } => {
                    self.errands.waits.push((talk, ticket, terminal));
                    continue;
                }
                Request::Kill { terminal } => {
                    self.errand(talk, &terminal).map_or_else(gone, |shell| {
                        shell.borrow_mut().kill();
                        Answer::Done
                    })
                }
                Request::Release { terminal } => self.release_errand(talk, &terminal),
            };
            if let Some(talk) = self.agents.get(talk) {
                talk.answer_request(ticket, answer);
            }
        }
        self.follow_errands();
    }

    /// Brings every agent's terminals up to date: the waits on the ones that
    /// have exited are answered, and the last lines of each are taken down
    /// for the tool calls that show them. Answers whether any changed.
    pub(super) fn follow_errands(&mut self) -> bool {
        let waits = std::mem::take(&mut self.errands.waits);
        for (talk, ticket, terminal) in waits {
            let answer = match self.errand(talk, &terminal) {
                None => gone(),
                Some(shell) => match exit(&mut shell.borrow_mut()) {
                    Some(exit) => Answer::Exited(exit),
                    None => {
                        self.errands.waits.push((talk, ticket, terminal));
                        continue;
                    }
                },
            };
            if let Some(talk) = self.agents.get(talk) {
                talk.answer_request(ticket, answer);
            }
        }

        let mut changed = false;
        for ((talk, terminal), (scope, id)) in &self.errands.shells {
            let Some(shell) = self.terminals.get(*scope, *id) else {
                continue;
            };
            let tail = shell.borrow().text();
            if let Some(talk) = self.agents.get_mut(*talk) {
                changed |= talk.show_terminal(terminal, tail);
            }
        }
        changed
    }

    /// Lets go of the terminals whose conversation has closed.
    ///
    /// Nobody is left to ask about them, so each is ended and leaves the list
    /// the way one the agent released would.
    pub(super) fn sweep_errands(&mut self) {
        let orphaned = self
            .errands
            .shells
            .keys()
            .filter(|(talk, _)| self.agents.get(*talk).is_none())
            .cloned()
            .collect::<Vec<_>>();
        for (talk, terminal) in orphaned {
            self.release_errand(talk, &terminal);
        }
        let agents = &self.agents;
        self.errands
            .waits
            .retain(|(talk, _, _)| agents.get(*talk).is_some());
    }

    /// Answers the request `talk` raised under `ticket` with what `answer`
    /// comes to, worked out away from the window.
    fn answer_later(
        &self,
        talk: TalkId,
        ticket: u64,
        answer: impl FnOnce() -> Answer + Send + 'static,
    ) {
        if let Some(talk) = self.agents.get(talk) {
            talk.answer_request_later(ticket, answer);
        }
    }

    /// The file at `path` as its open buffer in `scope` has it, or none when
    /// it is not open and has to be read from the disk.
    fn read_for_agent(&self, scope: Scope, path: &Path) -> Option<Answer> {
        let document = self
            .editor
            .opened(scope, path)
            .and_then(|id| self.editor.get(id))?;
        Some(Answer::Text(document.borrow().buffer().contents()))
    }

    /// Makes the file at `path` hold `text` through its open buffer in
    /// `scope`, or answers none when it is not open and has to be written to
    /// the disk.
    fn write_for_agent(&mut self, scope: Scope, path: &Path, text: &str) -> Option<Answer> {
        let id = self.editor.opened(scope, path)?;
        let root = self.root_of(scope)?;
        self.editor.write(id, text, &root);
        Some(Answer::Done)
    }

    /// Starts the command `run` names as one of `scope`'s shells, held for
    /// the conversation `talk` names.
    fn run_for_agent(&mut self, talk: TalkId, scope: Scope, run: Run) -> Answer {
        let mut env = self.worktree_env(scope);
        env.extend(run.env);
        match self
            .terminals
            .run(scope, &run.cwd, &run.command, &run.args, &env)
        {
            Ok(id) => {
                self.errands
                    .shells
                    .insert((talk, run.terminal), (scope, id));
                Answer::Done
            }
            Err(error) => Answer::Failed(error.to_string()),
        }
    }

    /// Ends the terminal `talk` calls `terminal` and lets it go, answering
    /// the waits on it with how it ended.
    fn release_errand(&mut self, talk: TalkId, terminal: &str) -> Answer {
        let Some((scope, id)) = self.errands.shells.remove(&(talk, terminal.to_owned())) else {
            return gone();
        };
        if let Some(shell) = self.terminals.get(scope, id) {
            let output = shell.borrow().text();
            if let Some(talk) = self.agents.get_mut(talk) {
                talk.show_terminal(terminal, output);
            }
            shell.borrow_mut().kill();
        }
        self.terminals.release(scope, id);
        let (released, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.errands.waits)
            .into_iter()
            .partition(|(waiter, _, named)| *waiter == talk && named == terminal);
        self.errands.waits = waiting;
        if let Some(talk) = self.agents.get(talk) {
            for (_, ticket, _) in released {
                talk.answer_request(ticket, Answer::Exited(Exit::default()));
            }
        }
        Answer::Done
    }

    /// The shell the conversation `talk` calls `terminal`, while it is listed.
    fn errand(&self, talk: TalkId, terminal: &str) -> Option<Shell> {
        let (scope, id) = self.errands.shells.get(&(talk, terminal.to_owned()))?;
        self.terminals.get(*scope, *id)
    }
}

/// How the command in `shell` ended, once it has.
fn exit(shell: &mut pm_vt::Terminal) -> Option<Exit> {
    if shell.is_running() {
        return None;
    }
    Some(Exit {
        code: shell.exit_code(),
        signal: shell.exit_signal(),
    })
}

/// The answer about a terminal that is no longer there.
///
/// The reader can stop any shell in the list, an agent's included, and the
/// agent asking after one they stopped is told so rather than left waiting.
fn gone() -> Answer {
    Answer::Failed("the terminal has been closed".to_owned())
}

/// The file at `path` as the disk has it.
fn read_from_disk(path: &Path) -> Answer {
    match pm_host::Host::local().fs().read_to_string(path) {
        Ok(text) => Answer::Text(text),
        Err(error) => Answer::Failed(error.to_string()),
    }
}

/// Makes the file at `path` on the disk hold `text`, making its folder if
/// it has none.
fn write_to_disk(path: &Path, text: &str) -> Answer {
    let written = path
        .parent()
        .map_or(Ok(()), |parent| {
            pm_host::Host::local().fs().create_dir_all(parent)
        })
        .and_then(|()| pm_host::Host::local().fs().write(path, text));
    match written {
        Ok(()) => Answer::Done,
        Err(error) => Answer::Failed(error.to_string()),
    }
}
