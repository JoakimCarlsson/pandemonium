//! What the window does about agents: starting one, and talking to it.
//!
//! An agent session is a pane like any other, so opening one is opening a tab
//! — but which agent to run, where to run it and who has the keyboard while it
//! is running are the window's, and they are here. Every command a session
//! answers to goes through [`App::agent_command`].

use std::path::{Path, PathBuf};

use pm_acp::{About, Agent, Knob, Setting};
use pm_text::Position;
use winit::window::UserAttentionType;

use crate::agent::{TalkId, Tally};
use crate::app::places::Place;
use crate::app::{App, Writing};
use crate::desktop;
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
            Message::ToggleAgentDetails(session, block) => {
                if let Some(talk) = self.agents.get_mut(session) {
                    talk.toggle_details(block);
                }
            }
            Message::FollowAgentLink(session, place) => self.follow_agent_link(session, place),
            Message::AttachAgentFiles(session) => self.attach_agent_files(session),
            Message::RemoveAgentAttachment(session, place) => {
                if let Some(talk) = self.agents.get_mut(session) {
                    talk.remove_attachment(place);
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
            Message::ShowAgentHistory(session) => self.show_agent_history(session),
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

    /// Opens a searchable list of this agent's saved sessions.
    pub(super) fn show_agent_history(&mut self, session: TalkId) {
        let Some(talk) = self.agents.get_mut(session).filter(|talk| talk.can_list()) else {
            return;
        };
        talk.list_history();
        let rows = self.agent_history_rows(session);
        self.open_picker_with(Kind::AgentHistory(session), rows, String::new());
    }

    /// Refreshes the open history picker as the agent returns its pages.
    pub(super) fn refresh_agent_history(&mut self) {
        let Some(Kind::AgentHistory(session)) = self.picker.as_ref().map(|picker| picker.kind())
        else {
            return;
        };
        let rows = self.agent_history_rows(session);
        if let Some(picker) = self.picker.as_mut() {
            picker.refill_preserving_selection(rows);
        }
    }

    /// Builds history choices from the saved sessions the agent has listed.
    pub(super) fn agent_history_rows(&self, session: TalkId) -> Vec<Row> {
        let Some(talk) = self.agents.get(session) else {
            return Vec::new();
        };
        let mut rows = talk
            .history()
            .iter()
            .map(|saved| Row {
                section: None,
                label: saved.title.clone().unwrap_or_else(|| saved.id.clone()),
                detail: saved
                    .updated_at
                    .as_deref()
                    .map_or_else(|| saved.id.clone(), |at| format!("{at} · {}", saved.id)),
                choice: Choice::AgentHistory(session, saved.id.clone()),
                enabled: true,
            })
            .collect::<Vec<_>>();
        let state = match (talk.history_error(), talk.is_listing(), rows.is_empty()) {
            (Some(error), _, _) => Some(error.to_owned()),
            (None, true, true) => Some("Loading sessions…".to_owned()),
            (None, true, false) => Some("Loading older sessions…".to_owned()),
            (None, false, true) => Some("No saved sessions in this worktree".to_owned()),
            (None, false, false) => None,
        };
        rows.extend(state.map(|label| Row {
            section: None,
            label,
            detail: String::new(),
            choice: Choice::AgentHistory(session, String::new()),
            enabled: false,
        }));
        rows
    }

    /// Loads a saved conversation into a tab of the same worktree.
    pub(super) fn open_agent_history(&mut self, source: TalkId, saved: &str) {
        let Some(talk) = self.agents.get(source) else {
            return;
        };
        let (scope, agent, root) = (talk.scope(), talk.agent(), talk.root().to_path_buf());
        if let Some(existing) = self.agents.find_saved(scope, agent, saved) {
            self.show_item(
                self.panes.focus(),
                scope,
                Item::Agent(scope, existing),
                false,
            );
            self.focus_prompt(existing);
            return;
        }
        let env = self.worktree_env(scope);
        let Some(opened) =
            self.agents
                .load(scope.project(), scope.session(), &root, &env, agent, saved)
        else {
            return;
        };
        self.show_item(self.panes.focus(), scope, Item::Agent(scope, opened), false);
        self.focus_prompt(opened);
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

    /// Stops the turn in the agent pane that has the keyboard.
    pub(super) fn cancel_busy_agent(&self) -> bool {
        let session = match self.writing {
            Some(Writing::Prompt(session)) => Some(session),
            _ if self.editor_focused => self.active_tab().and_then(Item::session),
            _ => None,
        };
        let Some(talk) = session.and_then(|session| self.agents.get(session)) else {
            return false;
        };
        if !talk.is_busy() {
            return false;
        }
        talk.cancel();
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

    /// Follows the link `session`'s pane drew in `place`.
    ///
    /// An agent names the files it talks about as links to them, relative to
    /// its worktree and often with a line after them; those open in the
    /// editor at that line, and anything else goes to the browser.
    fn follow_agent_link(&mut self, session: TalkId, place: usize) {
        let Some(talk) = self.agents.get(session) else {
            return;
        };
        let Some(link) = talk.drawn_link(place) else {
            return;
        };
        match linked_file(talk.root(), &link) {
            Some((path, line)) => {
                let place = Place {
                    scope: talk.scope(),
                    path,
                    position: Position::new(line, 0),
                };
                self.jump_to(&place);
            }
            None => desktop::browse(&link),
        }
    }

    /// Lets the reader choose files for this agent's next turn.
    fn attach_agent_files(&mut self, session: TalkId) {
        let Some(paths) = rfd::FileDialog::new()
            .set_title("Attach files")
            .pick_files()
        else {
            return;
        };
        if let Some(talk) = self.agents.get_mut(session) {
            for path in paths {
                talk.attach_file(path);
            }
        }
        self.focus_prompt(session);
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

    /// Scrolls the conversation the pointer is over `pixels` down, or up
    /// when negative.
    ///
    /// The wheel reports many times a frame, so it is held against how tall
    /// the pane last drew the conversation rather than wrapping all of it
    /// again for every step.
    ///
    /// The answer says whether there was one, so that the wheel goes on to
    /// whatever is behind it when there was not.
    pub(super) fn scroll_agent(&mut self, pixels: f32) -> bool {
        let Some(session) = self.agent_under() else {
            return false;
        };
        let Some(talk) = self.agents.get(session) else {
            return true;
        };
        let (drawn, view) = (talk.drawn_height().get(), talk.view().get().size);
        let end = match drawn > 0.0 && view.height > 0.0 {
            true => drawn - view.height,
            false => self.agent_end(session),
        };
        if let Some(talk) = self.agents.get_mut(session) {
            talk.scroll_by(pixels, end);
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
            let end = self.agent_end(session);
            if let Some(talk) = self.agents.get_mut(session) {
                talk.scroll_to(end);
            }
        }
    }

    /// How far `session` scrolls before its last row is against the foot of
    /// the pane, in logical pixels.
    ///
    /// The pane writes down where it drew the conversation; before it has,
    /// the focused pane less its bars is the best guess there is.
    fn agent_end(&self, session: TalkId) -> f32 {
        let Some(talk) = self.agents.get(session) else {
            return 0.0;
        };
        let theme = self.theme();
        let drawn = talk.view().get().size;
        let view = match drawn.height > 0.0 {
            true => drawn,
            false => {
                let pane = self
                    .geometry
                    .pane_size(self.panes.focus())
                    .unwrap_or_default();
                pm_gfx::Size::new(pane.width, pane.height - theme.size.tab_bar * 3.0)
            }
        };
        crate::agent::content_height(&theme, talk, view.width) - view.height
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

/// The file in the worktree at `root` that `link` names, and the line in it
/// counted from nought, where it names a file that is there.
///
/// The link is the agent's to write, so a file it names outside the worktree
/// — by an absolute path, or by climbing out through `..` or a link — is not
/// opened: the conversation is about the worktree it was started in.
///
/// A line is read from the `#L12` an address in a browser would carry, or
/// from the `:12` or `:12:4` a compiler writes after a path.
fn linked_file(root: &Path, link: &str) -> Option<(PathBuf, usize)> {
    let path = match link.split_once("://") {
        Some(("file", path)) => path,
        Some(_) => return None,
        None => link,
    };
    let (path, line) = match path.split_once("#L") {
        Some((path, line)) => (
            path,
            line.split('-').next().and_then(|line| line.parse().ok()),
        ),
        None => after_colons(path),
    };
    let path = root.join(path.replace("%20", " "));
    let resolved = path.canonicalize().ok()?;
    let inside = root
        .canonicalize()
        .is_ok_and(|root| resolved.starts_with(root));
    (inside && resolved.is_file()).then(|| (path, line.unwrap_or(1_usize).saturating_sub(1)))
}

/// `path` without the `:line` or `:line:column` written after it, and the
/// line, where one was.
fn after_colons(path: &str) -> (&str, Option<usize>) {
    match numbered(path) {
        Some((rest, last)) => match numbered(rest) {
            Some((file, line)) => (file, line.parse().ok()),
            None => (rest, last.parse().ok()),
        },
        None => (path, None),
    }
}

/// `path` split before the number written after its last colon, where a
/// number is what follows it.
fn numbered(path: &str) -> Option<(&str, &str)> {
    path.rsplit_once(':')
        .filter(|(_, number)| number.parse::<usize>().is_ok())
}
