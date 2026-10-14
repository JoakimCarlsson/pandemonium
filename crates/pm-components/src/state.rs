//! What the workspace remembers: the page on screen, the panels that are open and what was typed.

use pm_acp::{Event, Setting, Slash};
use pm_input::FocusId;
use pm_text::{Edit, Field};

use crate::context_menu::ContextMenu;
use crate::project::Project;
use crate::sample::{AGENTS, Change, Message, Session, Status, projects, sessions};

/// A prompt holding `text`, which may run to several lines.
pub fn prompt_field(text: &str) -> Field {
    Field::new(text).multiline(true)
}

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

/// Stops the turn the agent is on.
pub const STOP: FocusId = FocusId(322);

/// The conversation's scrolling area.
pub const CHAT_SCROLL: FocusId = FocusId(350);

/// The prompt field.
pub const PROMPT: FocusId = FocusId(320);

/// The commit message field.
pub const COMMIT_MESSAGE: FocusId = FocusId(321);

/// The first of the chips choosing an agent; the rest follow it.
pub const AGENT_BASE: u64 = 330;

/// The first of the rows folding a project; the rest follow it.
pub const PROJECT_BASE: u64 = 1000;

/// The first of the rows starting work in a project; the rest follow it.
pub const NEW_IN_BASE: u64 = 1100;

/// The first of the rows opening a session; the rest follow it.
pub const SESSION_BASE: u64 = 2000;

/// The first of the rows picking a changed file; the rest follow it.
pub const FILE_BASE: u64 = 3000;

/// The first of the check boxes staging a file; the rest follow it.
pub const STAGE_BASE: u64 = 3200;

/// The first of the check boxes marking a file viewed; the rest follow it.
pub const VIEWED_BASE: u64 = 3400;

/// The first of the buttons opening a setting's menu; the rest follow it.
pub const SETTING_BASE: u64 = 4200;

/// The first of the rows of the open setting's menu; the rest follow it.
pub const PICK_BASE: u64 = 4300;

/// The first of the rows of the commands offered behind a slash; the rest follow it.
pub const SLASH_BASE: u64 = 4400;

/// The first of the buttons choosing the effort; the rest follow it.
pub const EFFORT_BASE: u64 = 4500;

/// Opens the menu of where new work is done.
pub const LOCATION: FocusId = FocusId(4250);

/// The first of the rows choosing where new work is done; the rest follow it.
pub const PLACE_BASE: u64 = 4260;

/// The first of the rows choosing the branch new work is made from; the rest follow it.
pub const BRANCH_BASE: u64 = 4600;

/// Opens the agent's plan above the prompt box.
pub const PLAN_CHIP: FocusId = FocusId(4270);

/// Opens the questions waiting for an answer above the prompt box.
pub const APPROVALS_CHIP: FocusId = FocusId(4271);

/// The first of the buttons copying a reply; the rest follow it.
pub const COPY_BASE: u64 = 300_000;

/// The scrolling list of values in the open menu.
pub const MENU_SCROLL: FocusId = FocusId(351);

/// Switches the agent persona on or off.
pub const AGENT_TOGGLE: FocusId = FocusId(4290);

/// Puts a dollar sign in the prompt, which lists the skills the agent offers.
/// The button that attaches files to the prompt.
pub const ATTACH: FocusId = FocusId(318);

/// The first of the buttons that remove an attached file; the rest follow it.
pub const ATTACHED_BASE: u64 = 4800;

/// The button that starts mentioning a file in the prompt.
pub const MENTION: FocusId = FocusId(4281);

pub const SKILL_BUTTON: FocusId = FocusId(4282);

/// Puts a slash in the prompt, which lists the commands the agent offers.
/// The body of an open menu, which is not a button but keeps a click on it from counting as outside.
pub const MENU_BODY: FocusId = FocusId(4299);

/// The chip naming the account the agent is signed in as.
pub const ACCOUNT: FocusId = FocusId(4291);

/// The first of the rows choosing the account; the rest follow it.
pub const ACCOUNT_BASE: u64 = 4700;

pub const SLASH_BUTTON: FocusId = FocusId(4280);

/// The first of the lines folding the steps of a reply; the rest follow it.
pub const TURN_BASE: u64 = 100_000;

/// The first of the lines opening a step's detail; the rest follow it, a thousand to a reply.
pub const STEP_BASE: u64 = 200_000;

/// The first of the buttons answering the agent's question; the rest follow it.
pub const CHOICE_BASE: u64 = 3600;

/// Something the window has to do or tell an agent, taken off the state by whoever runs the agents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outgoing {
    /// Starts the agent of a new session, in a worktree off `base` when one is given, and hands it the words.
    Start {
        /// The session, by position.
        session: usize,
        /// What the reader wrote.
        text: String,
        /// The files attached, by path.
        files: Vec<String>,
        /// The settings the reader changed before starting, as the agent names them and their values.
        settings: Vec<(String, String)>,
        /// The branch to make the session's worktree from, when it works in one of its own.
        base: Option<String>,
    },
    /// Saves the named logins and the one in use for each agent, and points the agents at them.
    Accounts {
        /// The named logins.
        logins: Vec<(String, String, String)>,
        /// The login in use for each agent.
        in_use: Vec<(String, String)>,
    },
    /// Lists the files of the folder `directory` on `host`, for the list behind an `@`.
    Files {
        /// What the answer is held under, to tell whether it is still the one wanted.
        key: String,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The folder.
        directory: String,
    },
    /// Lists the branches of the repository `directory` on `host`, for the project at `project`.
    Branches {
        /// The project, by position.
        project: usize,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The folder.
        directory: String,
    },
    /// Lists the skills of the agent at `agent` in the project at `project`.
    Skills {
        /// The agent, by position.
        agent: usize,
        /// The project, by position.
        project: usize,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The project's folder.
        directory: String,
    },
    /// Asks the agent at `agent` on `host` what it lets the reader change and offers behind a slash.
    Discover {
        /// The agent, by position.
        agent: usize,
        /// The machine, or nothing for this one.
        host: Option<String>,
    },
    /// Asks who the agent at `agent` is signed in as, for naming the login in use after it when `naming`.
    Identity {
        /// The agent, by position.
        agent: usize,
        /// Whether the answer names the login in use.
        naming: bool,
    },
    /// Signs the agent, by its position, in under the login now in use.
    SignIn {
        /// The agent, by position.
        agent: usize,
    },
    /// Hands the agent of a session the words as its next turn.
    Prompt {
        /// The session, by position.
        session: usize,
        /// What the reader wrote.
        text: String,
        /// The files attached, by path.
        files: Vec<String>,
    },
    /// Answers the question the agent of a session asked.
    Choose {
        /// The session, by position.
        session: usize,
        /// The question, by the number the agent put it under.
        ticket: u64,
        /// The choice made.
        option: String,
    },
    /// Stops the turn the agent of a session is on.
    Cancel {
        /// The session, by position.
        session: usize,
    },
    /// Gives a setting of the agent of a session a value.
    Set {
        /// The session, by position.
        session: usize,
        /// What the agent calls the setting.
        setting: String,
        /// The value.
        value: String,
    },
    /// Stages or unstages files in the folder of a session.
    Stage {
        /// The session, by position.
        session: usize,
        /// The files, from the root of the repository.
        paths: Vec<String>,
        /// Whether to stage them, rather than unstage them.
        stage: bool,
    },
    /// Commits what is staged in the folder of a session.
    Commit {
        /// The session, by position.
        session: usize,
        /// The commit message.
        message: String,
    },
    /// Puts text on the clipboard.
    Copy {
        /// What to copy.
        text: String,
    },
}

/// What a menu over the prompt box is open for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Picker {
    /// A setting of the agent, by position among those on offer.
    Setting(usize),
    /// Where the work is done.
    Location,
    /// Which login of the agent is used.
    Account,
}

/// Where new work is done.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    /// In a worktree of its own, on a new branch.
    Worktree,
    /// In the project's own folder.
    Local,
}

/// A panel opened from the chips above the prompt box.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Activity {
    /// The agent's plan.
    Plan,
    /// The questions waiting for an answer.
    Approvals,
}

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

/// What one machine said about its agents: the settings and the commands of each.
pub type Discovered = (Vec<Vec<Setting>>, Vec<Vec<Slash>>);

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
    /// The files attached to the prompt, by path.
    pub attachments: Vec<String>,
    /// The machine, by destination, that the settings and commands held belong to; nothing for this one.
    pub discovery_host: Option<String>,
    /// What other machines said about the agents, by destination, while the settings of one machine are showing.
    pub discovered_elsewhere: std::collections::HashMap<Option<String>, Discovered>,
    /// What the attached pictures look like, by path, once they are decoded.
    pub previews: std::collections::HashMap<String, pm_ui::Picture>,
    /// The commit message being written.
    pub message: Field,
    /// How far the sidebar's list is scrolled, in logical pixels.
    pub sidebar_scroll: f32,
    /// The menu open over a field, and where, if one is.
    pub menu: Option<ContextMenu>,
    /// What the agents have yet to be told.
    pub outbox: Vec<Outgoing>,
    /// How far the conversation is scrolled back from its end, in logical pixels.
    pub chat_back: f32,
    /// The menu open over the prompt box, if one is.
    pub picker: Option<Picker>,
    /// What each agent lets the reader change, by the agent's position, once it has been asked.
    pub agent_settings: Vec<Vec<Setting>>,
    /// The settings the reader changed before starting work, by agent and name.
    pub touched: Vec<(usize, String)>,
    /// What the reader last chose for each setting of each agent: the agent, the setting and the value, as the agent names them.
    pub choices: Vec<(String, String, String)>,
    /// Where new work is done.
    pub location: Location,
    /// The branch new work is made from, when the reader picked one; otherwise the one checked out.
    pub base: Option<String>,
    /// The branches of the project being started in, the one checked out first; none when it is not a repository.
    pub branches: Vec<String>,
    /// The project `branches` was read for.
    pub branches_of: Option<usize>,
    /// The panel open over the conversation's chips, if one is.
    pub activity: Option<Activity>,
    /// The commands each agent offers behind a slash, by the agent's position, once it has been asked.
    pub agent_commands: Vec<Vec<Slash>>,
    /// The skills the chosen agent offers in the project being started in.
    pub skills: Vec<Slash>,
    /// The agent and project `skills` was read for.
    pub skills_of: Option<(usize, usize)>,
    /// The names of the skills installed for the chosen agent, which tell a skill from a command in the list behind a slash.
    pub skill_names: std::collections::HashSet<String>,
    /// The files of the folder the prompt is for, which an `@` in the prompt offers.
    pub files: Vec<String>,
    /// The folder `files` was read for.
    pub files_of: Option<String>,
    /// Who each agent is signed in as, by the agent's position, once it has been looked up.
    pub identities: Vec<Option<Option<String>>>,
    /// Who each agent is signed in as in its own default folder, by the agent's position, once it has been looked up.
    pub defaults: Vec<Option<Option<String>>>,
    /// The named logins the reader has added: the agent's id, the folder name and what it is called.
    pub logins: Vec<(String, String, String)>,
    /// The login in use for each agent, as the agent's id and the folder name; an agent not named uses its own.
    pub in_use: Vec<(String, String)>,
    /// How far the list of values in the open menu is scrolled, in logical pixels.
    pub menu_scroll: f32,
    /// The command highlighted in the list offered behind a slash, by position.
    pub slash: usize,
    /// The steps whose detail is open, by session, reply and position.
    pub opened: Vec<(usize, usize, usize)>,
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
        let mut state = Self {
            folded: vec![false; projects.len()],
            projects,
            sessions,
            page: Page::New(0),
            tab: Tab::Chat,
            review: true,
            terminal: false,
            agent: 0,
            file: None,
            prompt: prompt_field(""),
            message: Field::default(),
            sidebar_scroll: 0.0,
            menu: None,
            outbox: Vec::new(),
            chat_back: 0.0,
            picker: None,
            agent_settings: vec![Vec::new(); AGENTS.len()],
            touched: Vec::new(),
            choices: Vec::new(),
            location: Location::Local,
            base: None,
            branches: Vec::new(),
            branches_of: None,
            activity: None,
            agent_commands: vec![Vec::new(); AGENTS.len()],
            skills: Vec::new(),
            skills_of: None,
            skill_names: std::collections::HashSet::new(),
            attachments: Vec::new(),
            previews: std::collections::HashMap::new(),
            discovery_host: None,
            discovered_elsewhere: std::collections::HashMap::new(),
            files: Vec::new(),
            files_of: None,
            identities: vec![None; AGENTS.len()],
            defaults: vec![None; AGENTS.len()],
            logins: Vec::new(),
            in_use: Vec::new(),
            menu_scroll: 0.0,
            slash: 0,
            opened: Vec::new(),
        };
        state.sync_context();
        state
    }

    /// The workspace with what the reader chose for the agents' settings before.
    pub fn with_choices(mut self, choices: Vec<(String, String, String)>) -> Self {
        self.choices = choices;
        self
    }

    /// Adds `source` as a project unless it names no folder or is there already.
    pub fn with_project(mut self, source: &str) -> Self {
        if let Some(project) = Project::from_source(source)
            && !self.projects.iter().any(|held| held.path == project.path)
        {
            self.projects.push(project);
            self.folded.push(false);
        }
        self.sync_context();
        self
    }

    /// The state after the sidebar's list is scrolled by `delta` logical
    /// pixels, in a window `height` logical pixels tall, kept within the list.
    pub fn scrolled(mut self, delta: f32, height: f32) -> Self {
        let room = (self.list_height() - crate::sidebar::list_viewport(height)).max(0.0);
        self.sidebar_scroll = (self.sidebar_scroll + delta).clamp(0.0, room);
        self
    }

    /// How tall the sidebar's list of projects is, in logical pixels.
    fn list_height(&self) -> f32 {
        (0..self.projects.len())
            .map(|index| crate::sidebar::project_height(self, index))
            .sum()
    }

    /// Whether the page for starting work is showing, with a project to start it in.
    pub fn starting(&self) -> bool {
        matches!(self.page, Page::New(_)) && !self.projects.is_empty()
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
        if !Self::keeps_picker(action) {
            self.picker = None;
        }
        if !Self::keeps_activity(action) {
            self.activity = None;
        }
        match action {
            NEW => self.page = Page::New(self.current_project()),
            HISTORY => self.page = Page::History,
            TAB_CHAT => self.tab = Tab::Chat,
            TAB_DIFF => self.tab = Tab::Diff,
            TERMINAL => self.terminal = !self.terminal,
            REVIEW => self.review = !self.review,
            OPEN_DIFF => self.tab = Tab::Diff,
            SEND => self.send(),
            STOP => self.stop(),
            STAGE_ALL => self.stage_all(),
            COMMIT => self.commit(),
            FocusId(id) => self.after_numbered(id),
        }
        self.sync_context();
        self.refresh_files();
        self
    }

    /// The state after `edit` is made to the field named `id`.
    pub fn edited(mut self, id: FocusId, edit: Edit) -> Self {
        match id {
            PROMPT => {
                self.prompt.apply(edit);
                self.menu_scroll = 0.0;
            }
            COMMIT_MESSAGE => self.message.apply(edit),
            _ => {}
        }
        self.refresh_files();
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

    /// The state after the list of values in the open menu is scrolled by
    /// `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_menu(mut self, delta: f32, room: f32) -> Self {
        self.menu_scroll = (self.menu_scroll + delta).clamp(0.0, room);
        self
    }

    /// The state after the conversation is scrolled by `delta` logical
    /// pixels, downwards when positive, with `room` pixels to scroll back.
    pub fn scrolled_chat(mut self, delta: f32, room: f32) -> Self {
        self.chat_back = (self.chat_back - delta).clamp(0.0, room);
        self
    }

    /// Takes what the agents have yet to be told off the state.
    pub fn take_outbox(&mut self) -> Vec<Outgoing> {
        std::mem::take(&mut self.outbox)
    }

    /// The state after the agent of the session at `session` did `event`.
    pub fn heard(mut self, session: usize, event: Event) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            held.hear(event);
        }
        self
    }

    /// Whether an agent is at work, so the window should keep looking for what it says.
    pub fn waiting(&self) -> bool {
        self.sessions
            .iter()
            .any(|session| session.status == Status::Working)
    }

    /// The state after one of the rows or chips numbered from a base was pressed.
    fn after_numbered(&mut self, id: u64) {
        if self.press_choice(id) {
            return;
        }
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
            self.chat_back = 0.0;
        } else if let Some(index) = position(AGENT_BASE, AGENTS.len()) {
            self.agent = index;
        } else if let Some(index) = position(CHOICE_BASE, self.choices()) {
            self.choose(index);
        } else if let Some(index) = id
            .checked_sub(COPY_BASE)
            .map(|offset| offset as usize)
            .filter(|offset| *offset < self.messages_open())
        {
            self.copy(index);
        } else if let Some(index) = position(TURN_BASE, self.messages_open()) {
            self.toggle_turn(index);
        } else if id >= STEP_BASE {
            let offset = (id - STEP_BASE) as usize;
            self.toggle_step(offset / 1000, offset % 1000);
        } else if let Some(index) = position(FILE_BASE, self.change_count()) {
            self.file = Some(index);
            self.tab = Tab::Diff;
        } else if let Some(index) = position(STAGE_BASE, self.change_count()) {
            self.stage_one(index);
        } else if let Some(index) = position(VIEWED_BASE, self.change_count()) {
            self.change_at(index, |change| change.viewed = !change.viewed);
        }
    }

    /// The position of the open session, if the page is one.
    pub fn open_index(&self) -> Option<usize> {
        match self.page {
            Page::Session(index) => Some(index),
            _ => None,
        }
    }

    /// How many messages the open session has.
    fn messages_open(&self) -> usize {
        self.open().map_or(0, |session| session.messages.len())
    }

    /// Asks for the reply at `index` of the open session to be put on the clipboard.
    fn copy(&mut self, index: usize) {
        let Some(message) = self.open().and_then(|session| session.messages.get(index)) else {
            return;
        };
        let mut text = message.text.clone();
        for bullet in &message.bullets {
            text.push_str(&format!("\n- {bullet}"));
        }
        self.outbox.push(Outgoing::Copy { text });
    }

    /// Shows or hides the steps of the reply at `index` of the open session.
    fn toggle_turn(&mut self, index: usize) {
        let Page::Session(open) = self.page else {
            return;
        };
        let Some(session) = self.sessions.get_mut(open) else {
            return;
        };
        let latest = session.latest_steps();
        if let Some(message) = session.messages.get_mut(index) {
            message.open = Some(!message.open.unwrap_or(latest == Some(index)));
        }
    }

    /// Opens or closes the detail of step `step` of the reply at `message` of the open session.
    fn toggle_step(&mut self, message: usize, step: usize) {
        let Page::Session(open) = self.page else {
            return;
        };
        let key = (open, message, step);
        match self.opened.iter().position(|held| *held == key) {
            Some(at) => {
                self.opened.remove(at);
            }
            None => self.opened.push(key),
        }
    }

    /// How many answers the open session's agent is waiting on one of.
    fn choices(&self) -> usize {
        self.open()
            .and_then(|session| session.ask.as_ref())
            .map_or(0, |ask| ask.choices.len())
    }

    /// Answers the agent's question with its choice at `index`.
    fn choose(&mut self, index: usize) {
        let Page::Session(open) = self.page else {
            return;
        };
        let Some(ask) = self
            .sessions
            .get_mut(open)
            .and_then(|session| session.ask.take())
        else {
            return;
        };
        if let Some(choice) = ask.choices.get(index) {
            self.outbox.push(Outgoing::Choose {
                session: open,
                ticket: ask.ticket,
                option: choice.id.clone(),
            });
        }
    }

    /// Asks the open session's agent to stop what it is doing.
    fn stop(&mut self) {
        if let Page::Session(session) = self.page {
            self.outbox.push(Outgoing::Cancel { session });
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
    /// session, or as the first of a new one. Nothing is sent while the
    /// agent is still on a turn.
    fn send(&mut self) {
        let text = self.prompt.text().trim().to_owned();
        if text.is_empty() && self.attachments.is_empty() {
            return;
        }
        let session = match self.page {
            Page::Session(index) => {
                let Some(session) = self
                    .sessions
                    .get_mut(index)
                    .filter(|session| session.status != Status::Working)
                else {
                    return;
                };
                session.echoes += 1;
                let files = self.attachments.clone();
                let mut message = Message::user(&text);
                message.attachments = files.clone();
                session.messages.push(message);
                index
            }
            Page::New(project) => {
                let title: String = text.chars().take(40).collect();
                let mut session = Session::started(&title, project, &text);
                session.agent = self.agent;
                session.directory = self.projects[project]
                    .directory()
                    .to_string_lossy()
                    .into_owned();
                session.branch = self.branches.first().cloned().unwrap_or_default();
                session.base = self.base_for_new_work();
                session.skills = self.skills.clone();
                session.messages[0].attachments = self.attachments.clone();
                self.sessions.push(session);
                self.page = Page::Session(self.sessions.len() - 1);
                self.tab = Tab::Chat;
                self.sessions.len() - 1
            }
            Page::History => return,
        };
        self.prompt = prompt_field("");
        let files = std::mem::take(&mut self.attachments);
        self.chat_back = 0.0;
        self.slash = 0;
        let held = &mut self.sessions[session];
        held.status = Status::Working;
        held.updated = "now".to_owned();
        held.turn_started = Some(std::time::Instant::now());
        let outgoing = if self.sessions[session].messages.len() == 1 {
            Outgoing::Start {
                session,
                text,
                files,
                settings: self.settings_changed(),
                base: self.sessions[session].base.clone(),
            }
        } else {
            Outgoing::Prompt {
                session,
                text,
                files,
            }
        };
        self.outbox.push(outgoing);
    }

    /// Stages every change of the open session, or unstages them all when all are staged.
    fn stage_all(&mut self) {
        let Page::Session(open) = self.page else {
            return;
        };
        let Some(session) = self.sessions.get_mut(open) else {
            return;
        };
        let stage = !session.changes.iter().all(|change| change.staged);
        let paths: Vec<String> = session
            .changes
            .iter_mut()
            .filter(|change| change.staged != stage)
            .map(|change| {
                change.staged = stage;
                change.path.clone()
            })
            .collect();
        if !paths.is_empty() {
            self.outbox.push(Outgoing::Stage {
                session: open,
                paths,
                stage,
            });
        }
    }

    /// Stages the change at `index` of the open session, or unstages it when it is staged.
    fn stage_one(&mut self, index: usize) {
        let Page::Session(open) = self.page else {
            return;
        };
        let Some(change) = self
            .sessions
            .get_mut(open)
            .and_then(|session| session.changes.get_mut(index))
        else {
            return;
        };
        change.staged = !change.staged;
        let (paths, stage) = (vec![change.path.clone()], change.staged);
        self.outbox.push(Outgoing::Stage {
            session: open,
            paths,
            stage,
        });
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
        session.commits.push(message.clone());
        self.message = Field::default();
        self.file = None;
        self.outbox.push(Outgoing::Commit {
            session: index,
            message,
        });
    }

    /// The state after the folder of the session at `session` is ready: it
    /// works in `directory`, on `branch`.
    pub fn worktree_ready(mut self, session: usize, directory: &str, branch: &str) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            held.directory = directory.to_owned();
            held.branch = branch.to_owned();
        }
        self
    }

    /// The state after the folder of the session at `session` was looked at
    /// again: these files differ from the last commit, and these commits are its own.
    pub fn refreshed(
        mut self,
        session: usize,
        changes: &[pm_git::Change],
        commits: Option<Vec<String>>,
    ) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            held.changes = changes
                .iter()
                .map(|found| {
                    let mut change = Change::new(&found.path, &found.diff);
                    change.staged = found.staged;
                    change.viewed = held
                        .changes
                        .iter()
                        .any(|before| before.path == found.path && before.viewed);
                    change
                })
                .collect();
            if let Some(commits) = commits {
                held.commits = commits;
            }
            if held.status == Status::Done && !held.changes.is_empty() {
                held.status = Status::Review;
            }
        }
        if self.file.is_some_and(|file| file >= self.change_count()) {
            self.file = None;
        }
        self
    }

    /// The state after `text` is said in the conversation of the session at `session`, as the app and not the agent.
    pub fn noted(mut self, session: usize, text: &str) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            let mut message = Message::reply(text);
            message.duration = Some(String::new());
            held.messages.push(message);
            if held.status == Status::Working {
                held.status = Status::Done;
                held.turn_started = None;
            }
        }
        self
    }
}
