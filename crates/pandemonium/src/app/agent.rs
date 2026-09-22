//! What the window does about agents: starting one, and talking to it.
//!
//! An agent session is a pane like any other, so opening one is opening a tab
//! — but which agent to run, where to run it and who has the keyboard while it
//! is running are the window's, and they are here. Every command a session
//! answers to goes through [`App::agent_command`].

use pm_acp::Agent;

use crate::agent::SessionId;
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
            _ => return false,
        }
        true
    }

    /// The agents a reader can start, as the picker offers them.
    ///
    /// Every agent the editor knows about is offered, installed or not: one
    /// that is missing is fetched the first time it is started, and a list
    /// that left it out would be a list of what happens to be on this
    /// machine rather than of what the editor can run.
    pub(super) fn agent_rows(&self) -> Vec<Row> {
        pm_acp::AGENTS
            .into_iter()
            .map(|agent| Row {
                section: None,
                label: agent.name.to_owned(),
                detail: match agent.installed() {
                    true => agent.program.to_owned(),
                    false => format!("{} (fetched on first run)", agent.package),
                },
                choice: Choice::Agent(agent),
                enabled: true,
            })
            .collect()
    }

    /// Starts `agent` in the active project's worktree, and opens its pane.
    pub(super) fn start_agent(&mut self, agent: Agent) {
        let Some(project) = self.open.active() else {
            return;
        };
        let (project, root) = (project.id(), project.root().to_path_buf());
        let Some(session) = self.agents.start(project, &root, agent) else {
            return;
        };
        self.show_item(
            self.panes.focus(),
            project,
            Item::Agent(project, session),
            false,
        );
        self.focus_prompt(session);
    }

    /// Stops the turn the focused prompt is running, or lets go of it.
    ///
    /// One key gets out of one thing at a time: the first press stops the
    /// agent, and the press after it leaves the prompt — so a reader who
    /// wants the agent to stop never has to look at where the keyboard is.
    pub(super) fn stop_or_release_prompt(&mut self) -> bool {
        let Some(Writing::Prompt(session)) = self.writing else {
            return false;
        };
        match self.agents.get(session).filter(|talk| talk.is_busy()) {
            Some(talk) => talk.cancel(),
            None => self.writing = None,
        }
        true
    }

    /// Gives the keyboard to `session`'s prompt.
    pub(super) fn focus_prompt(&mut self, session: SessionId) {
        self.write_in(Writing::Prompt(session));
    }

    /// Sends what `session`'s prompt holds, and follows what comes back.
    pub(super) fn send_prompt(&mut self, session: SessionId) {
        if let Some(talk) = self.agents.get_mut(session) {
            talk.send();
        }
        self.focus_prompt(session);
        self.follow_agents();
    }

    /// The session the pointer is over, or the one the focused pane shows.
    fn agent_under(&self) -> Option<SessionId> {
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
    fn measure_agent(&self, session: SessionId) -> (usize, usize) {
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
