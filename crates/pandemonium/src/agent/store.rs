//! The conversations the window is holding, listed per project.
//!
//! [`Talks`] is the one seam an agent is started and ended through, and it
//! is keyed by project id the way the shells are: an agent works in the
//! worktree of the project it was started from, and a conversation that is
//! not attached to a project is a bug. Which worktree that is — the
//! project's own checkout, or a session cut beside it — is the window's to
//! say, and it says it by where the agent is started.
//!
//! [`Talk`] is one of them — the agent, everything said to it and by it, the
//! prompt being typed and whatever it is waiting to be allowed to do.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::agent::transcript::Transcript;
use crate::input::Input;
use pm_acp::{
    About, Agent, Ask, Command, Event, Knob, Mode, Notify, Session, Setting, Stop, Voice,
};
use pm_core::{ProjectId, SessionId};

/// A conversation's identity for as long as it is running.
///
/// Ids are handed out by [`Talks`] and are unique across the window, so a
/// tab, a keybinding and a pane all name the same conversation without
/// knowing which project it belongs to.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TalkId(u64);

/// One conversation: what is running, what has been said, what it is owed.
pub struct Talk {
    /// Which session this is.
    id: TalkId,
    /// The project whose worktree it is working in.
    project: ProjectId,
    /// The session it is working in, where it was started in one.
    session: Option<SessionId>,
    /// The conversation itself, as the protocol carries it.
    conversation: Session,
    /// Everything said so far.
    transcript: Transcript,
    /// The buffer the next prompt is written in.
    ///
    /// A prompt is several lines as often as it is one, so it is written in
    /// the editor the window is made of rather than in a line of its own.
    prompt: Input,
    /// The permission requests waiting on the reader, oldest first.
    asks: Vec<Ask>,
    /// The commands the agent has said it takes, as it last said them.
    commands: Vec<Command>,
    /// Which of the commands a slash narrows to is selected.
    chosen: usize,
    /// Whether the reader has waved that list away for what is typed now.
    dismissed: bool,
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
    pub fn id(&self) -> TalkId {
        self.id
    }

    /// Which agent is running.
    pub fn agent(&self) -> Agent {
        self.conversation.agent()
    }

    /// The worktree the agent is working in.
    pub fn root(&self) -> &Path {
        self.conversation.root()
    }

    /// What the agent calls this conversation, once it has opened one.
    pub fn resumable(&self) -> Option<String> {
        self.conversation.id()
    }

    /// Everything said so far.
    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// The box the next prompt is written in.
    pub fn prompt(&self) -> &Input {
        &self.prompt
    }

    /// That box, to write in.
    pub fn prompt_mut(&mut self) -> &mut Input {
        &mut self.prompt
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
        let typed = self.prompt.value();
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
        let Some(named) = self.naming().filter(|_| !self.dismissed) else {
            return Vec::new();
        };
        self.commands
            .iter()
            .filter(|command| command.name.to_lowercase().starts_with(&named))
            .collect()
    }

    /// Which of the offered commands is selected.
    ///
    /// The list is rebuilt on every keystroke and can grow shorter as it
    /// narrows, so the selection is held against what is offered now rather
    /// than trusted: a row that is no longer there selects the last one that
    /// is.
    pub fn chosen(&self) -> usize {
        self.chosen.min(self.offered().len().saturating_sub(1))
    }

    /// Moves the selection `by` rows through the offered commands.
    ///
    /// The ends are joined: a list a reader is stepping through is shorter
    /// than the reach of the key, and going up from the first row to the
    /// last is what every other list in the window does.
    pub fn step_command(&mut self, by: isize) {
        let offered = self.offered().len();
        if offered == 0 {
            return;
        }
        let at = self.chosen() as isize + by;
        self.chosen = at.rem_euclid(offered as isize) as usize;
    }

    /// Puts the selected command into the prompt.
    pub fn take_chosen(&mut self) {
        self.take_command(self.chosen());
    }

    /// Puts a slash in the prompt, which is what offers the commands.
    pub fn start_command(&mut self) {
        self.prompt.set("/");
        self.chosen = 0;
        self.dismissed = false;
    }

    /// Takes the list of commands away, leaving what has been typed alone.
    ///
    /// The list is a suggestion over the prompt, not a thing the prompt is
    /// in: waving it away leaves the slash, the rest of the line and the
    /// keyboard where they were. It comes back at the next keystroke.
    ///
    /// The answer says whether there was a list to take away.
    pub fn dismiss_commands(&mut self) -> bool {
        if self.offered().is_empty() {
            return false;
        }
        self.dismissed = true;
        true
    }

    /// Starts the selection again, for a prompt that has been typed into.
    pub fn retyped(&mut self) {
        self.chosen = 0;
        self.dismissed = false;
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
        self.prompt.set(&format!("/{command} "));
        self.chosen = 0;
        self.dismissed = false;
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

    /// The modes the agent takes, in the order it offered them.
    ///
    /// An agent says what its modes are when the conversation opens, and an
    /// agent that has none says nothing: an empty list is a session that is
    /// only ever in one mode, not one whose modes have not arrived.
    pub fn modes(&self) -> Vec<Mode> {
        self.conversation.modes()
    }

    /// What the mode it is in is called, which is what a reader is shown.
    ///
    /// A mode the agent has not named is shown as the agent named it: a
    /// session that has been put into a mode nobody listed is still in it.
    pub fn mode_name(&self) -> Option<String> {
        let mode = self.mode.as_deref()?;
        let named = self
            .modes()
            .into_iter()
            .find(|offered| offered.id == mode)
            .map(|offered| offered.name);
        Some(named.unwrap_or_else(|| mode.to_owned()))
    }

    /// Puts the session into the mode `mode` names.
    pub fn set_mode(&self, mode: &str) {
        self.conversation.set_mode(mode);
    }

    /// What the session can be set to, as the agent now offers it.
    ///
    /// A model, how hard the agent is made to think, a switch it offers and,
    /// for an agent that says so this way, the mode: the protocol has one
    /// shape for all of them, and so has the window.
    pub fn knobs(&self) -> Vec<Knob> {
        self.conversation.knobs()
    }

    /// The knob `id` names.
    pub fn knob(&self, id: &str) -> Option<Knob> {
        self.knobs().into_iter().find(|knob| knob.id == id)
    }

    /// The knob that is about `about`, where the agent offers one.
    pub fn knob_about(&self, about: About) -> Option<Knob> {
        self.knobs().into_iter().find(|knob| knob.about == about)
    }

    /// Sets the knob `id` names to the value `value` names.
    pub fn set_knob(&self, id: &str, value: &str) {
        self.conversation.set_knob(id, value);
    }

    /// Puts the switch `id` names the other way.
    pub fn toggle_knob(&self, id: &str) {
        let Some(Setting::Switched(on)) = self.knob(id).map(|knob| knob.setting) else {
            return;
        };
        self.conversation.switch_knob(id, !on);
    }

    /// Sets the knob `id` names to the value after the one it is set to.
    pub fn cycle_knob(&self, id: &str) {
        let Some(knob) = self.knob(id) else {
            return;
        };
        let Setting::Picked { value, picks } = knob.setting else {
            return self.toggle_knob(id);
        };
        if picks.is_empty() {
            return;
        }
        let at = picks
            .iter()
            .position(|pick| pick.id == value)
            .map_or(0, |at| (at + 1) % picks.len());
        self.set_knob(id, &picks[at].id);
    }

    /// Puts it into the mode after the one it is in, wrapping round.
    ///
    /// One chord walks the modes because there are two or three of them and
    /// a reader changing mode is usually going to the next one; the list is
    /// there for the times they are not.
    pub fn cycle_mode(&self) {
        let modes = self.modes();
        if modes.is_empty() {
            return;
        }
        let at = modes
            .iter()
            .position(|offered| Some(offered.id.as_str()) == self.mode())
            .map_or(0, |at| (at + 1) % modes.len());
        self.set_mode(&modes[at].id);
    }

    /// Whether the agent's process is still there.
    pub fn is_running(&self) -> bool {
        self.conversation.is_running()
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
        let text = self.prompt.value().trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.prompt.clear();
        self.transcript.say(Voice::Reader, &text);
        self.conversation.prompt(&text);
        self.chosen = 0;
        self.dismissed = false;
        self.busy = true;
        self.following = true;
    }

    /// Stops the turn that is running.
    pub fn cancel(&self) {
        self.conversation.cancel();
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
            Some(choice) => self.conversation.allow(ask, &choice.id),
            None => self.conversation.refuse(ask),
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
            Event::Knobs(_) => {}
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
                self.transcript.note(ended(&self.conversation));
            }
        }
    }
}

/// Every agent session the window is running.
#[derive(Default)]
pub struct Talks {
    /// The sessions, by the id each was handed.
    talks: BTreeMap<TalkId, Talk>,
    /// The id the next session started will be given.
    next: TalkId,
    /// How an agent wakes the window once it has something to say.
    notify: Option<Notify>,
    /// Whether a session has opened its conversation since this was asked.
    opened: bool,
}

impl Talks {
    /// Wakes the window through `notify` whenever an agent says something.
    pub fn set_notify(&mut self, notify: Notify) {
        self.notify = Some(notify);
    }

    /// Starts `agent` in `root` for `project`, and says which talk it is.
    pub fn start(
        &mut self,
        project: ProjectId,
        session: Option<SessionId>,
        root: &Path,
        agent: Agent,
    ) -> Option<TalkId> {
        self.open(project, session, root, agent, None)
    }

    /// Takes the conversation `resume` names up again, in a session of its own.
    pub fn resume(
        &mut self,
        project: ProjectId,
        session: Option<SessionId>,
        root: &Path,
        agent: Agent,
        resume: &str,
    ) -> Option<TalkId> {
        self.open(project, session, root, agent, Some(resume))
    }

    /// Starts `agent` in `root`, taking up `resume` where there is one.
    fn open(
        &mut self,
        project: ProjectId,
        session: Option<SessionId>,
        root: &Path,
        agent: Agent,
        resume: Option<&str>,
    ) -> Option<TalkId> {
        let notify = self.notify.clone()?;
        let started = match resume {
            Some(resume) => Session::resume(agent, root, resume, notify),
            None => Session::start(agent, root, notify),
        };
        let conversation = match started {
            Ok(conversation) => conversation,
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
        self.next = TalkId(id.0 + 1);
        self.talks.insert(
            id,
            Talk {
                id,
                project,
                session,
                conversation,
                transcript: Transcript::default(),
                prompt: Input::many_lines("Prompt"),
                asks: Vec::new(),
                commands: Vec::new(),
                chosen: 0,
                dismissed: false,
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

    /// The conversation running in the session `session` names, if one is.
    pub fn of_session(&self, session: SessionId) -> Option<TalkId> {
        self.talks
            .values()
            .find(|talk| talk.session == Some(session))
            .map(Talk::id)
    }

    /// The conversation `id` names.
    pub fn get(&self, id: TalkId) -> Option<&Talk> {
        self.talks.get(&id)
    }

    /// The conversation `id` names, to act on.
    pub fn get_mut(&mut self, id: TalkId) -> Option<&mut Talk> {
        self.talks.get_mut(&id)
    }

    /// Ends every session but the ones `held` names.
    ///
    /// An agent is a process, and the tab is the whole of what is holding it:
    /// a session nothing shows any more is one nobody can read, answer or
    /// stop, so it is ended rather than left running unseen.
    pub fn retain(&mut self, held: &BTreeSet<TalkId>) {
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
            for event in talk.conversation.drain() {
                self.opened |= matches!(event, Event::Ready);
                talk.take(event);
                changed = true;
            }
        }
        changed
    }

    /// Whether a conversation has been opened since this was last asked.
    ///
    /// The window writes down which conversation each pane is holding so
    /// that the next launch can take it up again, and the name to write down
    /// is the agent's, which does not exist until the agent has answered.
    pub fn take_opened(&mut self) -> bool {
        std::mem::take(&mut self.opened)
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
