//! The plain data the pages draw: projects, sessions, a conversation, the
//! git summary, which panels are open and which page shows.

use crate::browser::Browser;
use crate::browser_scope::BrowserShelf;
use crate::conversation::{Extras, Message};
use crate::dialog::{SignInCode, UpdateOffer};
use crate::dock_model::{DebugModel, Problem, TaskList, TerminalTab};
use crate::extensions::Extension;
use crate::file_tree::FileTree;
use crate::git::GitSummary;
use crate::history::PastSession;
use crate::open_files::OpenFiles;
use crate::page::{Page, Trail};
use crate::projects::{Adder, FolderPicker};
use crate::quick::QuickInput;
use crate::search::ProjectSearch;
use crate::terminal::TerminalScreen;

/// Everything one window shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Workspace {
    /// The projects in the sidebar, in their order.
    pub projects: Vec<Project>,
    /// The open session, as a path of indices: project, then nested projects, then the session.
    pub open: Option<Vec<usize>>,
    /// The title of the open session.
    pub title: String,
    /// The branch the open session works on.
    pub branch: String,
    /// The agent and account line in the session header.
    pub agent: String,
    /// The model the next prompt goes to.
    pub model: String,
    /// The permission mode shown in the composer.
    pub permissions: String,
    /// The account shown in the composer.
    pub account: String,
    /// The id of the login the open session runs as, or nothing for its agent's own.
    pub login: Option<String>,
    /// Whether a prompt sent to the open session waits for the server to take it.
    pub sending: bool,
    /// Where the open chat is to be continued, as picked so far.
    pub continuing: ContinueChoice,
    /// The machine the open session lives on, by host, or nothing for this one.
    pub machine: Option<String>,
    /// The named logins of the open session's agent the account chip offers, as id and name; empty when it offers none, as for a session of another machine.
    pub accounts: Vec<(String, String)>,
    /// The conversation of the open session.
    pub conversation: Vec<Message>,
    /// The cards, strips and bars around the open session's conversation.
    pub extras: Extras,
    /// The text typed into the composer and not yet sent.
    pub draft: String,
    /// The git state of the open session's worktree.
    pub git: GitSummary,
    /// Whether the sidebar is shown.
    pub sidebar_open: bool,
    /// The tab chosen in the right panel.
    pub right_tab: RightTab,
    /// Whether the right panel is shown.
    pub right_open: bool,
    /// The tab chosen in the bottom dock.
    pub dock_tab: DockTab,
    /// Whether the bottom dock is shown.
    pub dock_open: bool,
    /// What the shell in the dock shows.
    pub terminal: TerminalScreen,
    /// The shells of the folder the dock follows, as tabs, in order.
    pub terminals: Vec<TerminalTab>,
    /// The key of the shell shown, when there is one.
    pub terminal_shown: Option<u64>,
    /// The problems the language servers report, worst first.
    pub problems: Vec<Problem>,
    /// The tasks of the folder the dock follows.
    pub tasks: TaskList,
    /// How debugging the folder the dock follows stands.
    pub debug: DebugModel,
    /// A line telling what last went wrong, such as why the server cannot be reached or why a request failed, until the reader dismisses it.
    pub status: Option<String>,
    /// The session whose menu is open in the sidebar, when one is.
    pub session_menu: Option<u64>,
    /// The session being renamed and the title typed so far, while its dialog shows.
    pub renaming: Option<Renaming>,
    /// The session the reader asked to close although its worktree has changes not committed, while the question shows.
    pub closing: Option<u64>,
    /// How many agents work, while the question before the window closes on them shows.
    pub quitting: Option<usize>,
    /// Whether the reader said to close the window although agents work.
    pub quit_confirmed: bool,
    /// The welcome page for starting new work, while it is shown instead of a session.
    pub starting: Option<Start>,
    /// The agents new work can be started with, in the order the welcome page lists them.
    pub agents: Vec<AgentChoice>,
    /// The agent new work was last started with, or last picked, by its id.
    pub last_agent: Option<String>,
    /// How far the reader has read each session on this machine: its number and the last event read.
    pub read_through: std::collections::BTreeMap<u64, u64>,
    /// The page showing.
    pub page: Page,
    /// The pages visited, for going back and forward.
    pub trail: Trail,
    /// Every session the server holds, closed or not, newest first.
    pub history: Vec<PastSession>,
    /// The extensions the server lists, once it answered.
    pub extensions: Option<Vec<Extension>>,
    /// The extension in a folder the reader picked to install, while it is shown before installing.
    pub extension_preview: Option<crate::extensions::ExtensionPreview>,
    /// The latest commits of the project the overview shows, newest first.
    pub commits: Vec<Commit>,
    /// How big the resizable panels are.
    pub sizes: PanelSizes,
    /// The newer release offered, once the server found one.
    pub update: Option<UpdateOffer>,
    /// The code a sign-in waits for, while its dialog shows.
    pub sign_in: Option<SignInCode>,
    /// The files open in the session view and the tab chosen.
    pub open_files: OpenFiles,
    /// The file tree of the right panel's Files tab.
    pub tree: FileTree,
    /// The search of the right panel's Search tab.
    pub search: ProjectSearch,
    /// The quick input, while it is open.
    pub quick: Option<QuickInput>,
    /// The browser of the session shown, or of no session on the other pages, while it is open.
    pub browser: Option<Browser>,
    /// The browsers of the sessions not shown.
    pub shelf: BrowserShelf,
    /// The chats, conversations that belong to no project, newest first.
    pub chats: Vec<Session>,
    /// The address of the project whose menu is open, when one is.
    pub project_menu: Option<String>,
    /// The adder, while it is open.
    pub adder: Option<Adder>,
    /// The folder picker, while its window is open.
    pub picker: Option<FolderPicker>,
    /// Whether the projects shown are made up, standing in until the reader adds real ones.
    pub sample: bool,
}

/// A session being given a new title.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Renaming {
    /// The server's number for it.
    pub session: u64,
    /// The title typed so far.
    pub title: String,
}

/// How big the panels the reader resizes with sashes are, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PanelSizes {
    /// The sidebar's width.
    pub sidebar: f32,
    /// The right panel's width.
    pub right: f32,
    /// The dock's height.
    pub dock: f32,
}

impl PanelSizes {
    /// The narrowest the sidebar may be.
    pub const SIDEBAR_MIN: f32 = 180.0;
    /// The widest the sidebar may be.
    pub const SIDEBAR_MAX: f32 = 480.0;
    /// The narrowest the right panel may be.
    pub const RIGHT_MIN: f32 = 240.0;
    /// The widest the right panel may be.
    pub const RIGHT_MAX: f32 = 720.0;
    /// The lowest the dock may be.
    pub const DOCK_MIN: f32 = 90.0;
    /// The tallest the dock may be.
    pub const DOCK_MAX: f32 = 600.0;

    /// The sizes kept within the range each panel may take.
    pub fn clamped(self) -> Self {
        Self {
            sidebar: self.sidebar.clamp(Self::SIDEBAR_MIN, Self::SIDEBAR_MAX),
            right: self.right.clamp(Self::RIGHT_MIN, Self::RIGHT_MAX),
            dock: self.dock.clamp(Self::DOCK_MIN, Self::DOCK_MAX),
        }
    }
}

impl Default for PanelSizes {
    /// The sizes the window was drawn for.
    fn default() -> Self {
        Self {
            sidebar: 260.0,
            right: 330.0,
            dock: 170.0,
        }
    }
}

/// A commit of a project's repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Commit {
    /// Its abbreviated name.
    pub id: String,
    /// The first line of its message.
    pub summary: String,
}

/// What the welcome page starts: a session in a project, or a chat that belongs to none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Start {
    /// The index path of the project or submodule the session starts in, or nothing for a chat.
    pub at: Option<Vec<usize>>,
    /// The question the page asks, naming the project.
    pub heading: String,
    /// The position among [`Workspace::agents`] of the agent chosen.
    pub agent: usize,
    /// Whether the session works in a worktree of its own rather than in the project's checkout.
    pub worktree: bool,
    /// Whether its agent runs in the dev container of the folder it works in.
    pub container: bool,
    /// Whether the menu of where the work runs is open, for snapshots.
    pub place_menu: bool,
    /// The positions among [`Workspace::agents`] of the agents offered: those shown and signed in.
    pub shown: Vec<usize>,
    /// What the agent chosen lets the reader change, as it said when asked, with the values picked.
    pub settings: Vec<crate::composer::ConversationSetting>,
    /// The commands the agent chosen offers behind a slash.
    pub commands: Vec<crate::composer::SlashCommand>,
    /// The settings the reader changed, as the agent names them and their values.
    pub changed: Vec<(String, String)>,
    /// The branches of the project's repository, the one checked out first.
    pub branches: Vec<String>,
    /// The branch a worktree is made from, or nothing for what is checked out.
    pub base: Option<String>,
}

/// An agent new work can be started with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentChoice {
    /// The id the server knows it by.
    pub id: String,
    /// The name its chip shows.
    pub name: String,
    /// The provider the harness reaches its model through, which picks its glyph.
    pub provider: String,
    /// Whether it stands for a model server the reader declared.
    pub server: bool,
}

/// A project in the sidebar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    /// The name shown, usually the folder's.
    pub name: String,
    /// The folder or `ssh://host/folder` it lives at.
    pub path: String,
    /// Projects inside it, such as submodules, shown indented under it.
    pub children: Vec<Project>,
    /// Its sessions, newest first.
    pub sessions: Vec<Session>,
    /// Whether its rows are folded away.
    pub collapsed: bool,
    /// How the server of the machine it lives on stands, for a project on another machine.
    pub reach: Option<ProjectReach>,
}

impl Project {
    /// The machine it lives on, by host, or nothing for this one.
    pub fn machine(&self) -> Option<&str> {
        pm_protocol::host_of(&self.path).map(|(host, _)| host)
    }
}

/// Where the open chat is to be continued as a session, as picked so far.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContinueChoice {
    /// The project picked, as a path of indices among the projects.
    pub project: Option<Vec<usize>>,
    /// The branches of that project, once its machine listed them; none of them for a folder that holds no repository.
    pub branches: Option<Vec<String>>,
}

/// How the server of the machine a project lives on stands with the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectReach {
    /// It is being reached.
    Reaching,
    /// It answers.
    Reached,
    /// It cannot be reached, for this reason; the window tries again.
    Unreachable(String),
}

/// A session in the sidebar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    /// The server's number for it.
    pub id: u64,
    /// Its title.
    pub title: String,
    /// How long ago it last changed, such as `4d ago`.
    pub age: String,
    /// What it is doing.
    pub status: SessionStatus,
    /// The branch its worktree is on, or empty.
    pub branch: String,
    /// The folder its agent works in, as `ssh://host/folder` on another machine, or empty.
    pub directory: String,
}

/// What a session is doing, shown as its leading mark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionStatus {
    /// Waiting, nothing new.
    #[default]
    Idle,
    /// The agent is working.
    Working,
    /// It finished something not yet looked at.
    Unread,
    /// It needs an answer.
    NeedsInput,
    /// It failed.
    Error,
    /// It runs on another machine whose server cannot be reached now, so how it stands is not known.
    OutOfReach,
}

/// The tabs of the right panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RightTab {
    /// Source control.
    #[default]
    Git,
    /// The file tree.
    Files,
    /// Search across the project.
    Search,
}

/// The tabs of the bottom dock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DockTab {
    /// Diagnostics.
    Problems,
    /// A shell.
    #[default]
    Terminal,
    /// What the folder can be built, tested and run with.
    Tasks,
    /// Debugging the folder.
    Debug,
}
