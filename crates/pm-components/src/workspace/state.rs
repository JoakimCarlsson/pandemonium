//! What the workspace remembers: the page on screen, the panels that are open and what was typed.

use pm_input::FocusId;
use pm_text::{Edit, Field};

use super::sample::{AGENTS, Change, Message, SUGGESTIONS, Session, Status, projects, sessions};
use crate::context_menu::ContextMenu;
use crate::project::Project;

/// Starts a workspace in the project that is current.
pub const NEW: FocusId = FocusId(300);

/// Lists every session.
pub const HISTORY: FocusId = FocusId(301);

/// Searches the workspace.
pub const SEARCH: FocusId = FocusId(302);

/// Adds a project.
pub const ADD_PROJECT: FocusId = FocusId(303);

/// Opens the settings.
pub const SETTINGS: FocusId = FocusId(304);

/// Shows the conversation of the open session.
pub const TAB_CHAT: FocusId = FocusId(310);

/// Shows the diff of the open session.
pub const TAB_DIFF: FocusId = FocusId(311);

/// Shows or hides the terminal.
pub const TERMINAL: FocusId = FocusId(312);

/// Shows or hides the review panel.
pub const REVIEW: FocusId = FocusId(313);

/// Opens the diff from the summary of changes.
pub const OPEN_DIFF: FocusId = FocusId(314);

/// Hands the prompt to the agent.
pub const SEND: FocusId = FocusId(315);

/// Stages every change, or none when all are staged.
pub const STAGE_ALL: FocusId = FocusId(316);

/// Commits the staged changes.
pub const COMMIT: FocusId = FocusId(317);

/// The prompt field.
pub const PROMPT: FocusId = FocusId(320);

/// The commit message field.
pub const COMMIT_MESSAGE: FocusId = FocusId(321);

/// The first of the chips choosing an agent; the rest follow it.
pub(super) const AGENT_BASE: u64 = 330;

/// The first of the suggestions; the rest follow it.
pub(super) const SUGGESTION_BASE: u64 = 340;

/// The first of the rows folding a project; the rest follow it.
pub(super) const PROJECT_BASE: u64 = 1000;

/// The first of the rows starting work in a project; the rest follow it.
pub(super) const NEW_IN_BASE: u64 = 1100;

/// The first of the rows opening a session; the rest follow it.
pub(super) const SESSION_BASE: u64 = 2000;

/// The first of the rows picking a changed file; the rest follow it.
pub(super) const FILE_BASE: u64 = 3000;

/// The first of the check boxes staging a file; the rest follow it.
pub(super) const STAGE_BASE: u64 = 3200;

/// The first of the check boxes marking a file viewed; the rest follow it.
pub(super) const VIEWED_BASE: u64 = 3400;

/// What the main area shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Page {
    /// One session, by position.
    Session(usize),
    /// A prompt to start work in one project, by position.
    New(usize),
    /// Every session.
    History,
}

/// Which view of a session is showing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
    /// The conversation with the agent.
    Chat,
    /// The files the agent changed.
    Diff,
}

/// Everything the workspace window shows and lets the reader change.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceState {
    /// The projects, in the order the sidebar lists them.
    pub projects: Vec<Project>,
    /// Every session, across projects.
    pub sessions: Vec<Session>,
    /// What the main area shows.
    pub page: Page,
    /// Which view of the open session shows.
    pub tab: Tab,
    /// Whether each project's sessions are folded away.
    pub folded: Vec<bool>,
    /// Whether the review panel is open.
    pub review: bool,
    /// Whether the terminal is open.
    pub terminal: bool,
    /// The agent chosen for new work, by position.
    pub agent: usize,
    /// The changed file the reader picked, by position, if one was.
    pub file: Option<usize>,
    /// The prompt being written.
    pub prompt: Field,
    /// The commit message being written.
    pub message: Field,
    /// How far the sidebar's list is scrolled, in logical pixels.
    pub sidebar_scroll: f32,
    /// The menu open over a field, and where, if one is.
    pub menu: Option<ContextMenu>,
}

impl WorkspaceState {
    /// A workspace of `projects`, with no sessions yet, open on the page for starting work.
    pub fn new(projects: &[Project]) -> Self {
        Self::with(projects.to_vec(), Vec::new())
    }

    /// A workspace of made-up projects and sessions, open on the first session.
    pub fn sample() -> Self {
        Self {
            page: Page::Session(0),
            ..Self::with(projects(), sessions())
        }
    }

    /// A workspace of `projects` and `sessions`, open on the page for starting work.
    fn with(projects: Vec<Project>, sessions: Vec<Session>) -> Self {
        Self {
            folded: vec![false; projects.len()],
            projects,
            sessions,
            page: Page::New(0),
            tab: Tab::Chat,
            review: true,
            terminal: false,
            agent: 0,
            file: None,
            prompt: Field::default(),
            message: Field::default(),
            sidebar_scroll: 0.0,
            menu: None,
        }
    }

    /// Adds `source` as a project unless it names no folder or is there already.
    pub fn with_project(mut self, source: &str) -> Self {
        if let Some(project) = Project::from_source(source)
            && !self.projects.iter().any(|held| held.path == project.path)
        {
            self.projects.push(project);
            self.folded.push(false);
        }
        self
    }

    /// The state after the sidebar's list is scrolled by `delta` logical
    /// pixels, in a window `height` logical pixels tall, kept within the list.
    pub fn scrolled(mut self, delta: f32, height: f32) -> Self {
        let room = (self.list_height() - super::sidebar::list_viewport(height)).max(0.0);
        self.sidebar_scroll = (self.sidebar_scroll + delta).clamp(0.0, room);
        self
    }

    /// How tall the sidebar's list of projects is, in logical pixels.
    fn list_height(&self) -> f32 {
        (0..self.projects.len())
            .map(|index| super::sidebar::project_height(self, index))
            .sum()
    }

    /// The session on screen, if the page is one.
    pub fn open(&self) -> Option<&Session> {
        match self.page {
            Page::Session(index) => self.sessions.get(index),
            _ => None,
        }
    }

    /// The project the next new work goes to: the open session's, or the one being started in.
    pub fn current_project(&self) -> usize {
        match self.page {
            Page::New(project) => project,
            _ => self.open().map_or(0, |session| session.project),
        }
    }

    /// The state after `action` was pressed.
    pub fn after(mut self, action: FocusId) -> Self {
        match action {
            NEW => self.page = Page::New(self.current_project()),
            HISTORY => self.page = Page::History,
            TAB_CHAT => self.tab = Tab::Chat,
            TAB_DIFF => self.tab = Tab::Diff,
            TERMINAL => self.terminal = !self.terminal,
            REVIEW => self.review = !self.review,
            OPEN_DIFF => self.tab = Tab::Diff,
            SEND => self.send(),
            STAGE_ALL => self.stage_all(),
            COMMIT => self.commit(),
            FocusId(id) => self.after_numbered(id),
        }
        self
    }

    /// The state after `edit` is made to the field named `id`.
    pub fn edited(mut self, id: FocusId, edit: Edit) -> Self {
        match id {
            PROMPT => self.prompt.apply(edit),
            COMMIT_MESSAGE => self.message.apply(edit),
            _ => {}
        }
        self
    }

    /// The text field `id` names, if it names one.
    pub fn field(&self, id: FocusId) -> Option<&Field> {
        match id {
            PROMPT => Some(&self.prompt),
            COMMIT_MESSAGE => Some(&self.message),
            _ => None,
        }
    }

    /// Whether `id` names a text field.
    pub fn is_field(id: FocusId) -> bool {
        id == PROMPT || id == COMMIT_MESSAGE
    }

    /// What pressing the field `id` hands in when Enter is pressed in it.
    pub fn submit_of(id: FocusId) -> FocusId {
        if id == COMMIT_MESSAGE { COMMIT } else { SEND }
    }

    /// The state with a menu open over the field `field` at `at`.
    pub fn with_menu(mut self, field: FocusId, at: (f32, f32)) -> Self {
        self.menu = Some(ContextMenu { field, at });
        self
    }

    /// The state with no menu open.
    pub fn without_menu(mut self) -> Self {
        self.menu = None;
        self
    }

    /// The state after one of the rows or chips numbered from a base was pressed.
    fn after_numbered(&mut self, id: u64) {
        let position = |base: u64, count: usize| {
            id.checked_sub(base)
                .map(|offset| offset as usize)
                .filter(|offset| *offset < count)
        };
        if let Some(index) = position(PROJECT_BASE, self.projects.len()) {
            self.folded[index] = !self.folded[index];
        } else if let Some(index) = position(NEW_IN_BASE, self.projects.len()) {
            self.page = Page::New(index);
        } else if let Some(index) = position(SESSION_BASE, self.sessions.len()) {
            self.page = Page::Session(index);
            self.tab = Tab::Chat;
            self.file = None;
        } else if let Some(index) = position(AGENT_BASE, AGENTS.len()) {
            self.agent = index;
        } else if let Some(index) = position(SUGGESTION_BASE, SUGGESTIONS.len()) {
            self.prompt = Field::new(SUGGESTIONS[index].1);
            self.send();
        } else if let Some(index) = position(FILE_BASE, self.change_count()) {
            self.file = Some(index);
            self.tab = Tab::Diff;
        } else if let Some(index) = position(STAGE_BASE, self.change_count()) {
            self.change_at(index, |change| change.staged = !change.staged);
        } else if let Some(index) = position(VIEWED_BASE, self.change_count()) {
            self.change_at(index, |change| change.viewed = !change.viewed);
        }
    }

    /// How many changes the open session has.
    fn change_count(&self) -> usize {
        self.open().map_or(0, |session| session.changes.len())
    }

    /// Applies `edit` to the change at `index` of the open session.
    fn change_at(&mut self, index: usize, edit: impl FnOnce(&mut Change)) {
        if let Page::Session(open) = self.page
            && let Some(change) = self
                .sessions
                .get_mut(open)
                .and_then(|session| session.changes.get_mut(index))
        {
            edit(change);
        }
    }

    /// Hands the prompt to the agent: as the next message of the open
    /// session, or as the first of a new one.
    fn send(&mut self) {
        let text = self.prompt.text().trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.prompt = Field::default();
        match self.page {
            Page::Session(index) => {
                if let Some(session) = self.sessions.get_mut(index) {
                    session.messages.push(Message::user(&text));
                    session.status = Status::Working;
                    session.updated = "now".to_owned();
                }
            }
            Page::New(project) => {
                let title: String = text.chars().take(40).collect();
                let mut session = Session {
                    agent: self.agent,
                    ..Session::started(&title, project, &text)
                };
                session.updated = "now".to_owned();
                self.sessions.push(session);
                self.page = Page::Session(self.sessions.len() - 1);
                self.tab = Tab::Chat;
            }
            Page::History => {}
        }
    }

    /// Stages every change of the open session, or unstages them all when all are staged.
    fn stage_all(&mut self) {
        let all = self
            .open()
            .is_some_and(|session| session.changes.iter().all(|change| change.staged));
        for index in 0..self.change_count() {
            self.change_at(index, |change| change.staged = !all);
        }
    }

    /// Commits the staged changes of the open session under the message typed.
    fn commit(&mut self) {
        let message = self.message.text().trim().to_owned();
        let Page::Session(index) = self.page else {
            return;
        };
        let Some(session) = self.sessions.get_mut(index) else {
            return;
        };
        if message.is_empty() || !session.changes.iter().any(|change| change.staged) {
            return;
        }
        session.changes.retain(|change| !change.staged);
        session.commits.push(message);
        self.message = Field::default();
        self.file = None;
    }
}
