//! Where the window was when it was closed, kept on this machine so that the next launch opens on the same page.

use pm_components::state::{Page, SESSION_BASE, Tab, WorkspaceState, prompt_field};
use pm_input::FocusId;
use serde::{Deserialize, Serialize};

/// The name the view is kept under in the data folder.
const FILE: &str = "view";

/// The page the main area showed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
enum Saved {
    /// A session, by the machine it is on and the server's number for it.
    Session {
        /// The machine, by destination; nothing for this one.
        host: Option<String>,
        /// The server's number for the session.
        number: u64,
    },
    /// The page for starting work in the project at this path.
    New(String),
    /// Every session.
    History,
    /// The page for starting work in the first project.
    #[default]
    First,
}

/// What the reader had on screen: the page, the folded projects, the panels and the prompt being written.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// The page the main area showed.
    page: Saved,
    /// Whether the files the agent changed showed instead of the conversation.
    diff: bool,
    /// Whether the review panel was open.
    review: bool,
    /// Whether the terminal was open.
    terminal: bool,
    /// The paths of the projects whose sessions were folded away.
    folded: Vec<String>,
    /// How far the sidebar's list was scrolled, in logical pixels.
    scroll: f32,
    /// The agent chosen for new work, by position.
    agent: usize,
    /// The prompt being written.
    prompt: String,
}

impl View {
    /// The view saved by the last run, if there is one.
    pub fn load() -> Option<Self> {
        serde_json::from_str(&pm_store::load_text(FILE)?).ok()
    }

    /// Keeps the view for the next launch.
    pub fn save(&self) {
        if let Ok(text) = serde_json::to_string(self) {
            let _ = pm_store::save_text(FILE, &text);
        }
    }

    /// What `state` shows now, with `server_number` telling each session's machine and number on its server.
    pub fn capture(
        state: &WorkspaceState,
        server_number: impl Fn(usize) -> Option<(Option<String>, u64)>,
    ) -> Self {
        let path = |project: usize| {
            state
                .projects
                .get(project)
                .map(|held| held.path.clone())
                .unwrap_or_default()
        };
        let page = match state.page {
            Page::Session(position) => match server_number(position) {
                Some((host, number)) => Saved::Session { host, number },
                None => Saved::New(path(
                    state
                        .sessions
                        .get(position)
                        .map_or(0, |session| session.project),
                )),
            },
            Page::New(project) => Saved::New(path(project)),
            Page::History => Saved::History,
        };
        Self {
            page,
            diff: state.tab == Tab::Diff,
            review: state.review,
            terminal: state.terminal,
            folded: state
                .projects
                .iter()
                .zip(&state.folded)
                .filter(|(_, folded)| **folded)
                .map(|(project, _)| project.path.clone())
                .collect(),
            scroll: state.sidebar_scroll,
            agent: state.agent,
            prompt: state.prompt.text().to_owned(),
        }
    }

    /// The session that was open, by machine and the server's number, which can only be opened once the server has listed it.
    pub fn session(&self) -> Option<(Option<String>, u64)> {
        match &self.page {
            Saved::Session { host, number } => Some((host.clone(), *number)),
            _ => None,
        }
    }

    /// The machine whose server has to have listed its sessions before the view can be finished.
    pub fn home(&self) -> Option<String> {
        self.session().and_then(|(host, _)| host)
    }

    /// `state` with what does not wait for the server put back: the page for starting work or the history, the folded projects, the panels, the agent and the prompt.
    pub fn apply(&self, mut state: WorkspaceState) -> WorkspaceState {
        let project_at = |path: &String| state.projects.iter().position(|held| held.path == *path);
        match &self.page {
            Saved::New(path) => state.page = Page::New(project_at(path).unwrap_or(0)),
            Saved::History => state.page = Page::History,
            Saved::Session { .. } | Saved::First => {}
        }
        for (project, folded) in state.projects.iter().zip(state.folded.iter_mut()) {
            *folded = self.folded.contains(&project.path);
        }
        state.review = self.review;
        state.terminal = self.terminal;
        state.agent = self.agent.min(pm_acp::AGENTS.len().saturating_sub(1));
        state.prompt = prompt_field(&self.prompt);
        state
    }

    /// `state` open on the session at `position`, on the tab it was left on, with the sidebar scrolled as it was.
    pub fn reopen(&self, state: WorkspaceState, position: Option<usize>) -> WorkspaceState {
        let mut state = match position {
            Some(position) => {
                let mut opened = state.after(FocusId(SESSION_BASE + position as u64));
                opened.tab = if self.diff { Tab::Diff } else { Tab::Chat };
                opened
            }
            None => state,
        };
        state.sidebar_scroll = self.scroll;
        state
    }
}
