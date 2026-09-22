//! The agent sessions the window is holding, listed per project.
//!
//! [`Sessions`] is the one seam an agent is started and ended through, and it
//! is keyed by project id the way the shells are: an agent works in the
//! worktree of the project it was started from, and a session that is not
//! attached to a project is a bug.
//!
//! [`Talk`] is one of them — the agent, everything said to it and by it, the
//! prompt being typed and whatever it is waiting to be allowed to do.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::rc::Rc;

use crate::agent::transcript::Transcript;
use crate::editor::{Document, OpenFile};
use pm_acp::{Agent, Ask, Command, Event, Notify, Session, Stop, Voice};
use pm_core::ProjectId;

/// A session's identity for as long as it is running.
///
/// Ids are handed out by [`Sessions`] and are unique across the window, so a
/// tab, a keybinding and a pane all name the same conversation without
/// knowing which project it belongs to.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(u64);

/// One agent session: what it is, what has been said, and what it is owed.
pub struct Talk {
    /// Which session this is.
    id: SessionId,
    /// The project whose worktree it is working in.
    project: ProjectId,
    /// The agent itself.
    session: Session,
    /// Everything said so far.
    transcript: Transcript,
    /// The buffer the next prompt is written in.
    ///
    /// A prompt is several lines as often as it is one, so it is written in
    /// the editor the window is made of rather than in a line of its own.
    prompt: OpenFile,
    /// The permission requests waiting on the reader, oldest first.
    asks: Vec<Ask>,
    /// The commands the agent has said it takes, as it last said them.
    commands: Vec<Command>,
    /// Whether the conversation is open and will take prompts.
    ready: bool,
    /// Whether a turn is running.
    busy: bool,
    /// The mode the agent says it is in, where it has modes.
    mode: Option<String>,
    /// The first row the pane is drawn from.
    scroll: usize,
    /// Whether the pane follows the end of the conversation as it grows.
    following: bool,
}

impl Talk {
    /// Which session this is.
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// Which agent is running.
    pub fn agent(&self) -> Agent {
        self.session.agent()
    }

    /// The worktree the agent is working in.
    pub fn root(&self) -> &Path {
        self.session.root()
    }

    /// Everything said so far.
    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// The buffer the next prompt is written in.
    pub fn prompt(&self) -> OpenFile {
        self.prompt.clone()
    }

    /// The permission requests waiting on the reader.
    pub fn asks(&self) -> &[Ask] {
        &self.asks
    }

    /// What the prompt is naming after a slash, when that is what it holds.
    ///
    /// A command is only being named while it is the whole of the prompt: a
    /// slash with an argument after it has been named already, and a slash
    /// in the middle of a sentence is a slash.
    pub fn naming(&self) -> Option<String> {
        let typed = self.prompt.borrow().buffer().contents();
        let named = typed.strip_prefix('/')?;
        match named.contains(char::is_whitespace) {
            true => None,
            false => Some(named.to_lowercase()),
        }
    }

    /// The commands the prompt is narrowing to, while it is naming one.
    ///
    /// An agent says what it takes when the conversation opens and says it
    /// again whenever that changes, so this is what it offers now — the
    /// skills, the slash commands and whatever else it has put on the list.
    pub fn offered(&self) -> Vec<&Command> {
        let Some(named) = self.naming() else {
            return Vec::new();
        };
        self.commands
            .iter()
            .filter(|command| command.name.to_lowercase().starts_with(&named))
            .collect()
    }

    /// Puts the command in `place` of what is offered into the prompt.
    ///
    /// The command is left with a space after it and the turn is not sent:
    /// most of them take something, and the ones that do not are one more
    /// key away.
    pub fn take_command(&mut self, place: usize) {
        let Some(command) = self
            .offered()
            .get(place)
            .map(|command| command.name.clone())
        else {
            return;
        };
        self.prompt.borrow_mut().edit(|buffer| {
            buffer.select_all();
            buffer.delete();
            buffer.insert(&format!("/{command} "));
        });
    }

    /// Whether the conversation is open and will take prompts.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Whether a turn is running.
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    /// The mode the agent is in, where it has modes.
    pub fn mode(&self) -> Option<&str> {
        self.mode.as_deref()
    }

    /// Whether the agent's process is still there.
    pub fn is_running(&self) -> bool {
        self.session.is_running()
    }

    /// The first row the pane is drawn from.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Scrolls the pane by `rows`, of the `total` there are to show.
    ///
    /// Scrolling back is also what stops the pane following the end of the
    /// conversation: a reader who has gone up to read something is not
    /// dragged down again by the next thing the agent says.
    pub fn scroll_by(&mut self, rows: isize, total: usize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(rows)
            .min(total.saturating_sub(1));
        self.following = self.scroll + 1 >= total;
    }

    /// Puts the pane at `row`, which is what following the end comes to.
    pub fn scroll_to(&mut self, row: usize) {
        self.scroll = row;
    }

    /// Whether the pane follows the end of the conversation as it grows.
    pub fn is_following(&self) -> bool {
        self.following
    }

    /// Sends what is in the prompt buffer, and empties it.
    ///
    /// The prompt goes into the transcript here rather than when the agent
    /// echoes it: an agent is not obliged to say back what it was told, and
    /// a reader who has pressed Enter should see what they sent.
    pub fn send(&mut self) {
        let text = self.prompt.borrow().buffer().contents();
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.prompt.borrow_mut().edit(|buffer| {
            buffer.select_all();
            buffer.delete();
        });
        self.transcript.say(Voice::Reader, &text);
        self.session.prompt(&text);
        self.busy = true;
        self.following = true;
    }

    /// Stops the turn that is running.
    pub fn cancel(&self) {
        self.session.cancel();
    }

    /// Answers the permission request `ask` with the choice in `place`.
    ///
    /// A request the reader has answered leaves the list whichever way they
    /// answered it: the agent is waiting on one reply and gets one.
    pub fn answer(&mut self, ask: u64, place: usize) {
        let Some(at) = self.asks.iter().position(|waiting| waiting.id == ask) else {
            return;
        };
        let waiting = self.asks.remove(at);
        match waiting.choices.get(place) {
            Some(choice) => self.session.allow(ask, &choice.id),
            None => self.session.refuse(ask),
        }
    }

    /// Takes in one thing the agent said.
    fn take(&mut self, event: Event) {
        match event {
            Event::Ready => self.ready = true,
            Event::Login(methods) => {
                let names = methods
                    .iter()
                    .map(|method| method.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.transcript
                    .note(format!("{} needs logging in: {names}", self.agent().name));
            }
            Event::Said(voice, text) => self.transcript.say(voice, &text),
            Event::Ran(call) => self.transcript.ran(call),
            Event::Planned(steps) => self.transcript.planned(steps),
            Event::Offers(commands) => self.commands = commands,
            Event::Mode(mode) => self.mode = Some(mode),
            Event::Asked(ask) => self.asks.push(ask),
            Event::Stopped(stop) => {
                self.busy = false;
                if stop != Stop::EndTurn {
                    self.transcript.note(note(stop));
                }
            }
            Event::Failed(trouble) => {
                self.busy = false;
                self.transcript.note(trouble);
            }
            Event::Ended => {
                self.busy = false;
                self.ready = false;
                self.transcript.note(ended(&self.session));
            }
        }
    }
}

/// Every agent session the window is running.
#[derive(Default)]
pub struct Sessions {
    /// The sessions, by the id each was handed.
    talks: BTreeMap<SessionId, Talk>,
    /// The id the next session started will be given.
    next: SessionId,
    /// How an agent wakes the window once it has something to say.
    notify: Option<Notify>,
}

impl Sessions {
    /// Wakes the window through `notify` whenever an agent says something.
    pub fn set_notify(&mut self, notify: Notify) {
        self.notify = Some(notify);
    }

    /// Starts `agent` in `root` for `project`, and says which session it is.
    pub fn start(&mut self, project: ProjectId, root: &Path, agent: Agent) -> Option<SessionId> {
        let notify = self.notify.clone()?;
        let session = match Session::start(agent, root, notify) {
            Ok(session) => session,
            Err(error) => {
                eprintln!(
                    "could not start {} in {}: {error}",
                    agent.name,
                    root.display()
                );
                return None;
            }
        };

        let id = self.next;
        self.next = SessionId(id.0 + 1);
        self.talks.insert(
            id,
            Talk {
                id,
                project,
                session,
                transcript: Transcript::default(),
                prompt: Rc::new(RefCell::new(Document::scratch("Prompt"))),
                asks: Vec::new(),
                commands: Vec::new(),
                ready: false,
                busy: false,
                mode: None,
                scroll: 0,
                following: true,
            },
        );
        Some(id)
    }

    /// How many sessions `project` has running.
    pub fn count(&self, project: ProjectId) -> usize {
        self.talks
            .values()
            .filter(|talk| talk.project == project)
            .count()
    }

    /// The session `id` names.
    pub fn get(&self, id: SessionId) -> Option<&Talk> {
        self.talks.get(&id)
    }

    /// The session `id` names, to act on.
    pub fn get_mut(&mut self, id: SessionId) -> Option<&mut Talk> {
        self.talks.get_mut(&id)
    }

    /// Ends every session but the ones `held` names.
    ///
    /// An agent is a process, and the tab is the whole of what is holding it:
    /// a session nothing shows any more is one nobody can read, answer or
    /// stop, so it is ended rather than left running unseen.
    pub fn retain(&mut self, held: &BTreeSet<SessionId>) {
        self.talks.retain(|id, _| held.contains(id));
    }

    /// Ends every session of `project`, for a project leaving the window.
    pub fn close_project(&mut self, project: ProjectId) {
        self.talks.retain(|_, talk| talk.project != project);
    }

    /// Applies what every agent has said, and says whether anything changed.
    ///
    /// A session whose agent has gone is kept rather than dropped: what was
    /// said is still worth reading, and the tab closes when the reader closes
    /// it.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        for talk in self.talks.values_mut() {
            for event in talk.session.drain() {
                talk.take(event);
                changed = true;
            }
        }
        changed
    }
}

/// What the transcript says about a turn that ended some other way.
fn note(stop: Stop) -> &'static str {
    match stop {
        Stop::EndTurn => "",
        Stop::MaxTokens => "The agent ran out of tokens.",
        Stop::MaxRequests => "The agent ran out of turns.",
        Stop::Refusal => "The agent refused to go on.",
        Stop::Cancelled => "Stopped.",
    }
}

/// What the transcript says about an agent whose process has gone.
///
/// An agent that will not start says why on its error pipe and nowhere else,
/// so what it wrote there is what the reader is shown.
fn ended(session: &Session) -> String {
    let trouble = session.trouble();
    let last = trouble.lines().next_back().unwrap_or_default().trim();
    match last.is_empty() {
        true => "The agent has stopped.".to_owned(),
        false => format!("The agent has stopped: {last}"),
    }
}
