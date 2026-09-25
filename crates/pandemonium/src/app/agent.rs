//! What the window does about agents: starting one, and talking to it.
//!
//! An agent session is a pane like any other, so opening one is opening a tab
//! — but which agent to run, where to run it and who has the keyboard while it
//! is running are the window's, and they are here. Every command a session
//! answers to goes through [`App::agent_command`].

use pm_acp::{About, Agent, Knob, Setting};
use winit::window::UserAttentionType;

use crate::agent::{TalkId, Tally};
use crate::app::{App, Writing};
use crate::message::Message;
use crate::panes::Item;
use crate::picker::{Choice, Kind, Row};

impl App {
    /// Carries out the commands an agent session answers to.
    ///
    /// The answer says whether the message was one of them, so that the
    /// window can go on trying the rest.
    pub(super) fn agent_command(&mut self, message: Message) -> bool {
        match message {
            Message::NewAgentSession => self.open_picker(Kind::Agents),
            Message::WriteAgentPrompt(session, phase, anchor, head) => {
                self.point_in(Writing::Prompt(session), phase, anchor, head);
            }
            Message::SendPrompt(session) => self.send_prompt(session),
            Message::AnswerAgent(session, ask, place) => {
                if let Some(talk) = self.agents.get_mut(session) {
                    talk.answer(ask, place);
                }
            }
            Message::TakeAgentCommand(session, place) => {
                if let Some(talk) = self.agents.get_mut(session) {
                    talk.take_command(place);
                }
                self.focus_prompt(session);
            }
            Message::ShowAgentModes(session) => self.show_agent_modes(session),
            Message::CycleAgentMode(session) => self.cycle_agent_mode(session),
            Message::PressKnob(session, place) => self.press_knob(session, place),
            Message::StartAgentCommand(session) => {
                if let Some(talk) = self.agents.get_mut(session) {
                    talk.start_command();
                }
                self.focus_prompt(session);
            }
            Message::StartAgentSkill(session) => {
                if let Some(talk) = self.agents.get_mut(session) {
                    talk.start_skill();
                }
                self.focus_prompt(session);
            }
            Message::StopAgentTurn(session) => {
                if let Some(talk) = self.agents.get(session) {
                    talk.cancel();
                }
            }
            Message::ShowAgent(session) => self.show_agent(session),
            _ => return false,
        }
        true
    }

    /// The agents a reader can start, as the picker offers them.
    ///
    /// Every agent the editor knows about is listed, installed or not: one
    /// that is missing is fetched the first time it is started, and a list
    /// that left it out would be a list of what happens to be on this
    /// machine rather than of what the editor can run. An agent that comes
    /// from an installer instead of a package is listed unpickable until it
    /// is installed, with where to get it in its place.
    pub(super) fn agent_rows(&self) -> Vec<Row> {
        pm_acp::AGENTS
            .into_iter()
            .map(|agent| Row {
                section: None,
                label: agent.name.to_owned(),
                detail: match agent.installed() {
                    true => agent.program.to_owned(),
                    false => agent.source.hint(),
                },
                choice: Choice::Agent(agent),
                enabled: agent.startable(),
            })
            .collect()
    }

    /// Asks which mode to put `session` into.
    ///
    /// An agent says what its modes are in one of two ways — as modes, or as
    /// a knob that is about the mode — and both are asked about here, because
    /// to a reader there is one question. A session whose agent has neither
    /// has nothing to ask about, and the list is not opened on nothing.
    pub(super) fn show_agent_modes(&mut self, session: TalkId) {
        let rows = self.mode_rows(session);
        if !rows.is_empty() {
            return self.open_agent_choices(Kind::Modes, rows);
        }
        if let Some(place) = self.knob_about(session, About::Mode) {
            self.press_knob(session, place);
        }
    }

    /// Puts `session` into the mode after the one it is in.
    pub(super) fn cycle_agent_mode(&mut self, session: TalkId) {
        let Some(talk) = self.agents.get(session) else {
            return;
        };
        match talk.modes().is_empty() {
            false => talk.cycle_mode(),
            true => {
                if let Some(knob) = talk.knob_about(About::Mode) {
                    talk.cycle_knob(&knob.id);
                }
            }
        }
    }

    /// Does what pressing the knob in `place` of `session`'s means.
    ///
    /// A knob of several values asks which; a switch has two and is put the
    /// other way where it is shown, which is one press instead of two.
    pub(super) fn press_knob(&mut self, session: TalkId, place: usize) {
        let Some(knob) = self.knob_at(session, place) else {
            return;
        };
        let Setting::Picked { value, picks } = knob.setting else {
            if let Some(talk) = self.agents.get(session) {
                talk.toggle_knob(&knob.id);
            }
            return;
        };
        let rows = picks
            .into_iter()
            .map(|pick| Row {
                section: None,
                label: pick.name,
                detail: detail(pick.description.as_deref(), pick.id == value),
                choice: Choice::Knob(session, knob.id.clone(), pick.id),
                enabled: true,
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return;
        }
        self.open_agent_choices(Kind::Knob, rows);
    }

    /// Opens an agent control's choices beside the control that was pressed.
    fn open_agent_choices(&mut self, kind: Kind, rows: Vec<Row>) {
        self.agent_picker_at = self.pointer;
        self.open_picker_with(kind, rows, String::new());
        if let Some(picker) = self.picker.as_mut() {
            let current = picker
                .shown()
                .find_map(|(place, row)| row.detail.starts_with("current").then_some(place));
            if let Some(place) = current {
                picker.select(place);
            }
        }
    }

    /// The knob in `place` of what `session`'s agent offers.
    pub(super) fn knob_at(&self, session: TalkId, place: usize) -> Option<Knob> {
        self.agents.get(session)?.knobs().into_iter().nth(place)
    }

    /// Where the knob about `about` is, where `session`'s agent has one.
    pub(super) fn knob_about(&self, session: TalkId, about: About) -> Option<usize> {
        let talk = self.agents.get(session)?;
        talk.knobs().iter().position(|knob| knob.about == about)
    }

    /// Sets `session`'s knob `knob` to the value `value` names.
    pub(super) fn set_knob(&mut self, session: TalkId, knob: &str, value: &str) {
        if let Some(talk) = self.agents.get(session) {
            talk.set_knob(knob, value);
        }
    }

    /// The modes `session` can be put into, as the picker offers them.
    pub(super) fn mode_rows(&self, session: TalkId) -> Vec<Row> {
        let Some(talk) = self.agents.get(session) else {
            return Vec::new();
        };
        talk.modes()
            .into_iter()
            .map(|mode| {
                let current = Some(mode.id.as_str()) == talk.mode();
                Row {
                    section: None,
                    label: mode.name.clone(),
                    detail: detail(mode.description.as_deref(), current),
                    choice: Choice::Mode(session, mode.id),
                    enabled: true,
                }
            })
            .collect()
    }

    /// Puts `session` into the mode `mode` names.
    pub(super) fn set_agent_mode(&mut self, session: TalkId, mode: &str) {
        if let Some(talk) = self.agents.get(session) {
            talk.set_mode(mode);
        }
    }

    /// The conversation a command about an agent is about.
    ///
    /// It is the one being written to where a prompt has the keyboard, and
    /// the one the focused pane is showing otherwise: a reader who is typing
    /// at an agent means that agent, whichever pane is focused.
    pub(super) fn focused_talk(&self) -> Option<TalkId> {
        match self.writing {
            Some(Writing::Prompt(session)) => Some(session),
            _ => self.active_tab()?.session(),
        }
    }

    /// Starts `agent` where the window is pointed, and opens its pane.
    ///
    /// Where that is depends on what the reader has picked: the worktree of
    /// the session in hand, or the project's own checkout when they are in
    /// none. A session is cut before an agent is put in it, never by putting
    /// one in it.
    pub(super) fn start_agent(&mut self, agent: Agent) {
        let Some(project) = self.open.active() else {
            return;
        };
        let (project, checkout) = (project.id(), project.root().to_path_buf());
        let session = self.selected_session();
        let root = self.session_root().unwrap_or(checkout);
        self.open_agent(project, session, &root, agent);
    }

    /// Starts `agent` in `root` for `project`, and opens the pane it is read in.
    ///
    /// Every agent the window starts comes through here, whichever worktree
    /// it is started in, so a conversation, its tab and the keyboard always
    /// arrive together.
    pub(super) fn open_agent(
        &mut self,
        project: pm_core::ProjectId,
        session: Option<pm_core::SessionId>,
        root: &std::path::Path,
        agent: Agent,
    ) {
        let scope = match session {
            Some(session) => pm_core::Scope::of(project, session),
            None => pm_core::Scope::checkout(project),
        };
        let env = self.worktree_env(scope);
        let Some(talk) = self.agents.start(project, session, root, &env, agent) else {
            return;
        };
        self.show_item(self.panes.focus(), scope, Item::Agent(scope, talk), false);
        self.focus_prompt(talk);
    }

    /// Gets out of one thing the focused prompt is in the middle of.
    ///
    /// One key gets out of one thing at a time, nearest the reader first: the
    /// list of commands a slash put up, then the turn that is running, then
    /// the prompt itself — so a reader who wants the agent to stop never has
    /// to look at where the keyboard is, and one who was only shown a list
    /// keeps what they had typed.
    pub(super) fn stop_or_release_prompt(&mut self) -> bool {
        let Some(Writing::Prompt(session)) = self.writing else {
            return false;
        };
        if let Some(talk) = self.agents.get_mut(session)
            && talk.dismiss_commands()
        {
            return true;
        }
        match self.agents.get(session).filter(|talk| talk.is_busy()) {
            Some(talk) => talk.cancel(),
            None => self.writing = None,
        }
        true
    }

    /// Gives the keyboard to `session`'s prompt.
    pub(super) fn focus_prompt(&mut self, session: TalkId) {
        self.write_in(Writing::Prompt(session));
    }

    /// Sends what `session`'s prompt holds, and follows what comes back.
    pub(super) fn send_prompt(&mut self, session: TalkId) {
        if let Some(talk) = self.agents.get_mut(session) {
            talk.send();
        }
        self.focus_prompt(session);
        self.follow_agents();
    }

    /// The session the pointer is over, or the one the focused pane shows.
    fn agent_under(&self) -> Option<TalkId> {
        let scope = self.scope()?;
        let pane = self
            .pointer
            .and_then(|at| self.geometry.pane_at(at))
            .unwrap_or_else(|| self.panes.focus());
        self.panes.pane(pane)?.active(scope)?.session()
    }

    /// Scrolls the conversation the pointer is over by `rows`.
    ///
    /// The answer says whether there was one, so that the wheel goes on to
    /// whatever is behind it when there was not.
    pub(super) fn scroll_agent(&mut self, rows: isize) -> bool {
        let Some(session) = self.agent_under() else {
            return false;
        };
        let (total, held) = self.measure_agent(session);
        if let Some(talk) = self.agents.get_mut(session) {
            talk.scroll_by(rows, total.saturating_sub(held.saturating_sub(1)));
        }
        true
    }

    /// Marks every conversation a pane is showing as read, while the window
    /// has the reader's attention.
    ///
    /// A turn that ended in a pane on screen has been seen; one that ended
    /// behind another tab, or while the reader was in another application,
    /// is still news.
    pub(super) fn see_shown_agents(&mut self) {
        if !self.window_focused {
            return;
        }
        let shown = self
            .panes
            .panes()
            .into_iter()
            .filter_map(|pane| self.panes.pane(pane)?.active(self.scope()))
            .filter_map(Item::session)
            .collect::<Vec<_>>();
        for session in shown {
            if let Some(talk) = self.agents.get_mut(session) {
                talk.see();
            }
        }
    }

    /// Asks the desktop to point the reader at the window when an agent has
    /// started waiting on them, stopped or finished a turn while they were
    /// elsewhere.
    ///
    /// A question blocks the agent until it is answered, so it is asked for
    /// until the window is focused; a finished turn is mentioned once.
    pub(super) fn call_reader(&self, before: Tally) {
        if self.window_focused {
            return;
        }
        let after = self.agents.tally();
        let urgency = match () {
            () if after.waiting > before.waiting => UserAttentionType::Critical,
            () if after.stopped > before.stopped => UserAttentionType::Informational,
            () if after.done > before.done => UserAttentionType::Informational,
            () => return,
        };
        if let Some(window) = self.window.as_ref() {
            window.request_user_attention(Some(urgency));
        }
    }

    /// Keeps every conversation that is following its end at its end.
    ///
    /// Following is what a terminal does: the last thing said stays against
    /// the foot of the pane and everything above it scrolls off. How much of
    /// the conversation that leaves showing is what the pane came out at last
    /// frame, so the pane's own height is what the first row is counted from.
    pub(super) fn follow_agents(&mut self) {
        let sessions = self
            .panes
            .panes()
            .into_iter()
            .filter_map(|pane| self.panes.pane(pane))
            .flat_map(crate::panes::Pane::items)
            .filter_map(Item::session)
            .collect::<Vec<_>>();

        for session in sessions {
            if self
                .agents
                .get(session)
                .is_none_or(|talk| !talk.is_following())
            {
                continue;
            }
            let (total, held) = self.measure_agent(session);
            if let Some(talk) = self.agents.get_mut(session) {
                talk.scroll_to(total.saturating_sub(held));
            }
        }
    }

    /// How many rows `session` comes to, and how many of them a pane holds.
    fn measure_agent(&self, session: TalkId) -> (usize, usize) {
        let theme = self.theme();
        let size = self
            .geometry
            .pane_size(self.panes.focus())
            .unwrap_or_default();
        let line = theme.text.code.line_height.max(1.0);
        let held = ((size.height - theme.size.tab_bar * 3.0) / line).max(1.0) as usize;
        let total = self
            .agents
            .get(session)
            .map_or(0, |talk| crate::agent::row_count(&theme, talk, size.width));
        (total, held)
    }
}

/// What a mode's row says beside its name.
///
/// The mode the session is already in says so, because a list of modes with
/// nothing marked is a list a reader has to remember their way around.
fn detail(description: Option<&str>, current: bool) -> String {
    match (description.unwrap_or_default(), current) {
        ("", true) => "current".to_owned(),
        (description, true) => format!("current · {description}"),
        (description, false) => description.to_owned(),
    }
}
