//! The link to the server on this machine: asks for the projects, their
//! submodules and the sessions, follows the open session's events, and keeps
//! a [`Workspace`] in step with what it hears, including which page shows
//! and the trail of pages visited; and reads and saves what the
//! settings window shows of the server, keeping a [`SettingsModel`].

mod composer_requests;
mod conversation_requests;
mod sessions;
mod start_requests;
pub use start_requests::BASE_SETTING;

pub use conversation_requests::ConversationCommand;
pub use keys::KeyOutcome;
pub use sessions::SessionCommand;

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use pm_client::{Client, Incoming, Wake};
use pm_harness::Event;
use pm_protocol::{Request, Response, SessionInfo};

use crate::composer::ComposerState;
use crate::conversation::Extras;
use crate::dialog::{SignInCode, UpdateOffer, UpdateRun};
use crate::dock::{Dock, DockAsk};
use crate::dock_model::DebugCommand;
use crate::extensions::{ExtensionAction, listed};
use crate::history::PastSession;
use crate::model::{Commit, Project, Session, Workspace};
use crate::page::Page;
use crate::sample::sample_workspace;
use crate::settings_change::{SettingsChange, release_request};
use crate::settings_model::Release;
use crate::settings_model::SettingsModel;
use crate::start::{offered_agents, show_start, start_chat, start_in, title_of};
use crate::thread::Thread;

mod browse;
mod browser;
mod browser_agent;
mod code;
mod excerpts;
mod file_actions;
mod file_keys;
mod git;
mod keys;
mod listing;
mod lsp;
mod machines;
mod projects;
mod quick_input;
mod remote;
mod review;
mod routes;
mod timeline;
mod watches;

use crate::view::View;
use code::CodeAsk;
use excerpts::ExcerptAsk;
use git::GitAsk;
use lsp::ServerFor;
use remote::Remotes;
use routes::Routes;

/// What an outstanding request was for.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ask {
    /// The list of projects.
    Projects,
    /// The list of sessions.
    Sessions,
    /// The submodules of the project at this address.
    Submodules(String),
    /// Something the dock asked for.
    Dock(DockAsk),
    /// Starting a session, which opens once the server started it.
    Start,
    /// The latest commits of the project at this address.
    Log(String),
    /// What the server keeps for the work it does on its machine.
    MachineSettings,
    /// The extensions, languages among them.
    Extensions,
    /// The themes, icon themes and commands the installed extensions bring.
    ExtensionAssets,
    /// The tool servers.
    McpServers,
    /// The model servers.
    ModelServers,
    /// The model slots and presets.
    ModelSlots,
    /// The customizations of the agent in the project.
    Library,
    /// The tools of the agent.
    ToolSets,
    /// The newest release.
    LatestRelease,
    /// The pull request of the open session's branch.
    PullRequest,
    /// Downloading a newer release.
    Update,
    /// Something the Git panel asked.
    Git(GitAsk),
    /// Something about the open files.
    Code(CodeAsk),
    /// Something asked of the language server of an open file.
    Server(ServerFor),
    /// The files of the folder at this address, for the file tree.
    Files(String),
    /// Making, renaming, moving, copying, importing or removing an entry of the file tree.
    Entry,
    /// The lines of the project holding this text, for the Search tab.
    Search(String),
    /// What the quick input lists for this query.
    Quick(String),
    /// The branches of the open session's repository, for the quick input.
    Branches,
    /// The model slots of this session's agent, for its model picker.
    SessionSlots(u64),
    /// How much of the allowance of this session's account is used, for its composer's ring.
    SessionAllowance(u64),
    /// What a pick list of the quick input holds.
    Listing(crate::quick::Choosing),
    /// Something the Search tab's editable excerpts asked.
    Excerpt(ExcerptAsk),
    /// The folders inside a folder, for the folder picker, on the machine with this host or this one.
    Folders(Option<String>),
    /// Cloning a repository from the adder.
    Cloned,
    /// The sessions the server of the machine with this host holds.
    RemoteSessions(String),
    /// Something a form of the settings window sent.
    Form,
    /// Branching a new session off a turn of the open one, which opens once the server made it.
    Fork,
    /// Closing the session the window numbers so, which asks before throwing away changes not committed.
    Close(u64),
    /// Who the agent with this id is signed in as, for the welcome page.
    Identity(String),
    /// What the agent with this id lets the reader change, for the welcome page.
    Discover(String),
    /// The branches of the repository at this folder, for the welcome page.
    StartBranches(String),
    /// Nothing that needs handling but a failure, which the status bar shows.
    Nothing,
}

impl Ask {
    /// What answering `request` is for.
    fn of(request: &Request) -> Self {
        match request {
            Request::MachineSettings => Self::MachineSettings,
            Request::Extensions => Self::Extensions,
            Request::ExtensionAssets => Self::ExtensionAssets,
            Request::McpServers => Self::McpServers,
            Request::ModelServers | Request::RemoveModelServer { .. } => Self::ModelServers,
            Request::ModelSlots { .. } => Self::ModelSlots,
            Request::Library { .. } => Self::Library,
            Request::ToolSets { .. } => Self::ToolSets,
            Request::LatestRelease { .. } => Self::LatestRelease,
            Request::Accounts
            | Request::SetAccounts { .. }
            | Request::SaveKey { .. }
            | Request::SignIn { .. }
            | Request::SearchTools { .. }
            | Request::FindModels { .. }
            | Request::CreateCustomization { .. } => Self::Form,
            Request::SaveModelServer { .. } => Self::ModelServers,
            Request::PullRequest { .. } | Request::PullRequestAction { .. } => Self::PullRequest,
            _ => Self::Nothing,
        }
    }
}

/// The connection to the local server and the workspace built from it.
pub struct Link {
    /// The connection, once the server was found.
    client: Option<Client>,
    /// What each outstanding request was for, by id.
    asks: HashMap<u64, Ask>,
    /// The project addresses, oldest first.
    paths: Vec<String>,
    /// The submodules of each project, by its address.
    submodules: HashMap<String, Vec<String>>,
    /// Every session the server knows.
    sessions: Vec<SessionInfo>,
    /// The open session.
    watched: Option<u64>,
    /// The folders each server was last told to watch, by its machine's host, `None` for this machine.
    folder_watches: std::collections::BTreeMap<Option<String>, Vec<String>>,
    /// The timeline of the file it was last asked for, as far as the server answered.
    timeline: timeline::Timeline,
    /// What each session's events built, by session.
    threads: HashMap<u64, Thread>,
    /// The titles of sessions whose agent finished a turn live, not yet taken.
    finished: Vec<String>,
    /// The shells, problems, tasks and debugging the dock shows.
    dock: Dock,
    /// The closed sessions the reader opened again from the history, listed in the sidebar once more.
    reopened: HashSet<u64>,
    /// The workspace as it now stands.
    workspace: Workspace,
    /// What the settings window shows and edits.
    settings: SettingsModel,
    /// The page the window was left on, opened once the server lists its sessions.
    pending: Option<Page>,
    /// The files open when the window was left, opened again once the
    /// server lists its sessions.
    reopen: Vec<crate::open_files::SavedFile>,
    /// What the language servers last found wrong with each file, by folder
    /// and path, handed to a file's editor when it opens.
    diagnosed: std::collections::HashMap<(String, String), Vec<pm_protocol::Diagnostic>>,
    /// Where the server put a downloaded release, not yet taken to be installed.
    staged: Option<String>,
    /// The engine behind the browser pane.
    web: crate::web::Web,
    /// The connections to the servers of other machines.
    remotes: Remotes,
    /// The addresses of the projects the reader folded.
    folded: HashSet<String>,
    /// Whether the reader ever saved projects; until then the sidebar shows made-up ones.
    saved: bool,
    /// The sessions the server of each other machine holds, by host.
    remote_sessions: HashMap<String, Vec<SessionInfo>>,
    /// The numbers the window gives sessions, shells and debugging sessions of other machines.
    routes: Routes,
    /// The chords pressed so far of a binding of several, waiting for the next.
    chords: Vec<pm_editor::Chord>,
    /// Whether each agent is signed in, by its id, as the server said when asked.
    identities: HashMap<String, bool>,
    /// The files opened lately from the quick input, the latest first.
    recent_files: Vec<String>,
    /// The titles of the commands run lately from the quick input, the latest first.
    recent_commands: Vec<String>,
}

impl Link {
    /// Connects to the server on this machine, starting it when it is not
    /// running, and calls `wake` whenever something arrives.
    pub fn connect(wake: Wake) -> Self {
        let (client, status) = match pm_client::local() {
            Ok(target) => (Some(Client::connect(target, wake.clone())), None),
            Err(error) => (None, Some(error)),
        };
        Self::built(client, status, wake)
    }

    /// A link over `client`, or none, with `status` in the status line, calling `wake` whenever something arrives.
    fn built(client: Option<Client>, status: Option<String>, wake: Wake) -> Self {
        let layout = sample_workspace();
        let mut workspace = Workspace {
            page: Page::Home,
            sidebar_open: layout.sidebar_open,
            right_tab: layout.right_tab,
            right_open: layout.right_open,
            dock_tab: layout.dock_tab,
            dock_open: layout.dock_open,
            sizes: layout.sizes,
            ..Workspace::default()
        };
        workspace.agents = offered_agents();
        workspace.git.branch = String::new();
        let dock = Dock::default();
        dock.publish(&mut workspace);
        let web = crate::web::Web::new(wake.clone());
        workspace.status = status;
        Self {
            client,
            asks: HashMap::new(),
            paths: Vec::new(),
            submodules: HashMap::new(),
            sessions: Vec::new(),
            watched: None,
            folder_watches: std::collections::BTreeMap::new(),
            timeline: timeline::Timeline::default(),
            threads: HashMap::new(),
            finished: Vec::new(),
            dock,
            reopened: HashSet::new(),
            workspace,
            settings: SettingsModel::new(Default::default(), Default::default()),
            pending: None,
            reopen: Vec::new(),
            diagnosed: std::collections::HashMap::new(),
            staged: None,
            web,
            remotes: Remotes::new(wake),
            folded: HashSet::new(),
            saved: true,
            remote_sessions: HashMap::new(),
            routes: Routes::default(),
            chords: Vec::new(),
            identities: HashMap::new(),
            recent_files: Vec::new(),
            recent_commands: Vec::new(),
        }
    }

    /// Shows or hides the dialog offering the newer release; hiding it stops a download under way.
    pub fn show_update(&mut self, open: bool) {
        let Some(offer) = &mut self.workspace.update else {
            return;
        };
        offer.open = open;
        if !open && matches!(offer.run, UpdateRun::Downloading { .. }) {
            offer.run = UpdateRun::Idle;
            self.send(Request::CancelUpdate);
        }
    }

    /// Asks the server to download the newer release offered, to be installed once it is ready.
    pub fn start_update(&mut self) {
        let Some(offer) = &mut self.workspace.update else {
            return;
        };
        if offer.run.busy() {
            return;
        }
        offer.run = UpdateRun::Downloading {
            received: 0,
            total: 0,
        };
        let version = offer.version.clone();
        let Request::LatestRelease { channel } =
            release_request(self.settings.window.general.channel)
        else {
            return;
        };
        self.ask(Request::DownloadUpdate { channel, version }, Ask::Update);
    }

    /// Takes where the server put the downloaded release, once it is ready to be installed.
    pub fn take_staged(&mut self) -> Option<String> {
        self.staged.take()
    }

    /// Tells the reader installing the newer release failed for `reason`.
    pub fn update_failed(&mut self, reason: String) {
        if let Some(offer) = &mut self.workspace.update {
            offer.run = UpdateRun::Failed(reason);
        }
    }

    /// Hides the dialog showing the code a sign-in waits for.
    pub fn hide_sign_in(&mut self) {
        self.workspace.sign_in = None;
    }

    /// Lays the window out the way `view` says and opens its page once the server lists its sessions.
    pub fn restore(&mut self, view: View) {
        view.lay_out(&mut self.workspace);
        self.reopen = view.files.clone();
        self.pending = Some(Page::from(view.page));
    }

    /// Opens the page the window was left on, if it is still waiting and still there.
    fn open_pending(&mut self) {
        let Some(page) = self.pending.take() else {
            return;
        };
        self.workspace.open_files.vim = self.settings.editor.vim_mode;
        for saved in std::mem::take(&mut self.reopen) {
            if self.workspace.open_files.reopen(&saved).is_some() {
                self.read_file(saved.directory, saved.path);
            }
        }
        let there = match &page {
            Page::Session(id) => self.sessions.iter().any(|info| info.session == *id),
            Page::Overview(path) => self.paths.contains(path),
            _ => true,
        };
        if there && page != Page::Home {
            self.navigate(page);
        }
    }

    /// The workspace as it now stands.
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// The workspace, to change what only the window decides, such as which panels show.
    pub fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspace
    }

    /// What the settings window shows and edits.
    pub fn settings(&self) -> &SettingsModel {
        &self.settings
    }

    /// The settings model, to put in what the window read of its own, such as its settings files and fonts.
    pub fn settings_mut(&mut self) -> &mut SettingsModel {
        &mut self.settings
    }

    /// Readies the settings window as it opens: names the agent and the project its customizations are read for, the open session's or else the first, on whichever machine it lives, and asks the server for what its pages list.
    pub fn open_settings(&mut self) {
        let agent = self
            .settings
            .machine
            .agents
            .default_agent
            .clone()
            .or_else(|| self.settings.agents.first().map(|agent| agent.id.clone()));
        let open_project = self
            .workspace
            .open
            .as_ref()
            .filter(|_| !self.workspace.sample)
            .and_then(|at| self.workspace.projects.get(*at.first()?))
            .map(|project| project.path.clone());
        let project = open_project.or_else(|| self.paths.first().cloned());
        self.settings.library_place = agent.zip(project.map(|path| expand_home(&path)));
        let mut requests = vec![
            Request::McpServers,
            Request::ModelServers,
            Request::Extensions,
            Request::ExtensionAssets,
        ];
        requests.extend(self.settings.asks_for(self.settings.page.section));
        if self.settings.window.general.check_updates && self.settings.release.is_none() {
            requests.push(release_request(self.settings.window.general.channel));
        }
        for request in requests {
            self.send(request);
        }
    }

    /// Takes in `change` the settings window asked for and sends the server what it needs.
    pub fn change_settings(&mut self, change: SettingsChange) {
        for request in self.settings.apply(change) {
            if matches!(request, Request::SaveMachineSettings { .. }) {
                self.share_machine_settings();
            }
            self.send(request);
        }
        self.change_dock(|_| {});
    }

    /// Sends `request`, remembering what its answer is for; the agent's tools and what is withheld from it go to the machine of the project the settings window reads customizations for.
    fn send(&mut self, request: Request) {
        let ask = Ask::of(&request);
        let library_host = self
            .settings
            .library_place
            .as_ref()
            .and_then(|(_, directory)| pm_protocol::host_of(directory))
            .map(|(host, _)| host.to_owned());
        match (library_host, &request) {
            (Some(host), Request::ToolSets { .. } | Request::SaveWithheld { .. }) => {
                self.ask_remote(&host, request, ask)
            }
            _ => self.ask(request, ask),
        }
    }

    /// Handles everything the server sent since the last call and returns
    /// whether the workspace changed.
    pub fn pump(&mut self) -> bool {
        let remote = self.pump_remotes();
        let incoming = self
            .client
            .as_ref()
            .map(Client::incoming)
            .unwrap_or_default();
        let changed = !incoming.is_empty();
        for message in incoming {
            self.handle(message);
        }
        let browsed = self.run_web();
        self.browser_agent();
        self.sync_watches();
        browsed || changed || remote
    }

    /// Opens the session at index path `at`, recording it in the trail.
    pub fn open_session(&mut self, at: Vec<usize>) {
        if let Some(session) = session_at(&self.workspace.projects, &at) {
            self.navigate(Page::Session(session.id));
        }
    }

    /// Opens the session numbered `id` from the history, listing it in the
    /// sidebar again when the reader had closed it.
    pub fn reopen_session(&mut self, id: u64) {
        self.reopened.insert(id);
        self.rebuild();
        self.navigate(Page::Session(id));
    }

    /// Shows `page` and records it in the trail.
    pub fn navigate(&mut self, page: Page) {
        self.workspace.trail.visit(page.clone());
        self.go(page);
    }

    /// Goes back to the page before the one showing in the trail.
    pub fn back(&mut self) {
        if let Some(page) = self.workspace.trail.back() {
            self.go(page);
        }
    }

    /// Goes forward to the page after the one showing in the trail.
    pub fn forward(&mut self) {
        if let Some(page) = self.workspace.trail.forward() {
            self.go(page);
        }
    }

    /// Installs, updates or uninstalls the extension `id`, as its button
    /// says, unless it is being installed.
    pub fn press_extension(&mut self, id: &str) {
        let Some(extension) = self
            .workspace
            .extensions
            .iter()
            .flatten()
            .find(|extension| extension.id == id)
        else {
            return;
        };
        let request = match extension.action {
            ExtensionAction::Install | ExtensionAction::Update => Request::InstallExtensions {
                ids: vec![extension.id.clone()],
            },
            ExtensionAction::Uninstall => Request::RemoveExtension {
                id: extension.id.clone(),
            },
            ExtensionAction::Installing => return,
        };
        self.ask(request, Ask::Nothing);
    }

    /// Shows the welcome page for a new session in the project or submodule
    /// at index path `at`.
    pub fn new_session(&mut self, at: Vec<usize>) {
        if let Some(start) = start_in(&self.workspace.projects, &at) {
            self.watched = None;
            show_start(&mut self.workspace, start);
            self.rescope_browser();
            self.prepare_start();
        }
    }

    /// Shows the welcome page for a new chat, which belongs to no project.
    pub fn new_chat(&mut self) {
        self.watched = None;
        show_start(&mut self.workspace, start_chat());
        self.rescope_browser();
        self.prepare_start();
    }

    /// Makes the work the welcome page starts run in a worktree of its own when `worktree`, otherwise in the project's checkout, and in the folder's dev container when `container`.
    pub fn choose_place(&mut self, worktree: bool, container: bool) {
        if let Some(start) = &mut self.workspace.starting {
            start.worktree = worktree;
            start.container = container;
            start.place_menu = false;
        }
    }

    /// Asks the server to start the work the welcome page describes with
    /// `text` as its first prompt; the session opens once it is started.
    pub fn start(&mut self, text: String) {
        let Some(start) = self.workspace.starting.clone() else {
            return;
        };
        let Some(agent) = self
            .workspace
            .agents
            .get(start.agent)
            .map(|agent| agent.id.clone())
        else {
            return;
        };
        let (repository, submodule) = match &start.at {
            Some(at) => match self.placement(at) {
                Some(placement) => placement,
                None => return,
            },
            None => (String::new(), None),
        };
        let title = title_of(&text);
        let composer = &mut self.workspace.extras.composer;
        let text = crate::composer::prompt_text(&text, &composer.elements, &composer.captures);
        let attachments = composer
            .files
            .iter()
            .filter_map(composer_requests::attachment_of)
            .collect();
        composer.files.clear();
        composer.elements.clear();
        composer.captures.clear();
        self.workspace.last_agent = Some(agent.clone());
        self.ask(
            Request::StartSession {
                agent,
                directory: repository.clone(),
                repository,
                title: title.clone(),
                base: (start.worktree && start.at.is_some())
                    .then(|| start.base.clone().unwrap_or_else(|| "HEAD".to_owned())),
                settings: start.changed.clone(),
                text,
                attachments,
                submodule,
                container: start.container && start.at.is_some(),
                scratch: start.at.is_none(),
            },
            Ask::Start,
        );
        self.workspace.title = title;
    }

    /// The project folder and submodule a session started at index path `at` works in.
    fn placement(&self, at: &[usize]) -> Option<(String, Option<String>)> {
        let path = self.paths.get(*at.first()?)?;
        let submodule = match at.get(1) {
            Some(index) => Some(self.submodules.get(path)?.get(*index)?.clone()),
            None => None,
        };
        Some((expand_home(path), submodule))
    }

    /// Shows `page` without recording it, asking the server for what it lists.
    fn go(&mut self, page: Page) {
        self.workspace.starting = None;
        match &page {
            Page::Home => {
                self.workspace.open = None;
                self.watched = None;
                self.workspace.title = String::new();
                self.workspace.conversation = Vec::new();
            }
            Page::Session(id) => self.show_session(*id),
            Page::History => {}
            Page::Extensions => self.send(Request::Extensions),
            Page::Overview(path) => {
                self.workspace.commits = Vec::new();
                if pm_protocol::host_of(path).is_none() {
                    self.ask(
                        Request::Log {
                            directory: expand_home(path),
                            limit: COMMITS_SHOWN,
                            all: false,
                        },
                        Ask::Log(path.clone()),
                    );
                }
            }
        }
        self.workspace.page = page;
        self.rescope_browser();
        self.refresh_browse();
    }

    /// Shows the session numbered `id`, leaving the welcome page, and follows its events.
    fn show_session(&mut self, id: u64) {
        let info = self.session_info(id);
        let info = info.as_ref();
        if let Some(info) = info {
            self.workspace.title = info.title.clone();
        }
        self.workspace.branch = info
            .and_then(|info| info.branch.clone())
            .unwrap_or_else(|| "main".to_owned());
        self.workspace.agent = info.map(|info| info.agent.clone()).unwrap_or_default();
        self.workspace.account = info.and_then(|info| info.login.clone()).unwrap_or_default();
        self.workspace.starting = None;
        self.workspace.extras = Default::default();
        self.workspace.extras.chat = info.is_some_and(|info| info.chat);
        self.watched = Some(id);
        self.swap_open_files(id);
        let folder = info
            .filter(|info| !info.chat)
            .map(|info| (info.directory.clone(), info.base.clone()))
            .unwrap_or_default();
        let branch = self.workspace.branch.clone();
        self.git_open(folder.0, folder.1, branch);
        if let Some(info) = info.filter(|info| !info.directory.is_empty()) {
            let folder = info.directory.clone();
            self.change_dock(|dock| dock.follow(folder));
        }
        let thread = self.threads.entry(id).or_default();
        thread.mark_read();
        if self.routes.remote(id).is_none() {
            self.workspace.read_through.insert(id, thread.seq);
        }
        show_thread(&mut self.workspace, thread);
        let agent = self.workspace.agent.clone();
        let login = info.and_then(|info| info.login.clone());
        self.ask_composer(id, agent, login);
        self.ask_pull_request();
        self.rebuild();
    }

    /// Sends `text` as a prompt to the open session.
    pub fn send_prompt(&mut self, text: String) {
        self.prompt(text, false);
    }

    /// Changes the dock with `change`, sends what it asks of the server, has the connection replay and follow the shells it took up, and shows how it stands.
    fn change_dock(&mut self, change: impl FnOnce(&mut Dock)) {
        self.dock
            .set_scrollback(self.settings.window.terminal.scrollback);
        change(&mut self.dock);
        for (request, ask) in self.dock.take_requests() {
            self.ask(request, Ask::Dock(ask));
        }
        for terminal in self.dock.take_watches() {
            if let Some(client) = &self.client {
                client.watch_terminal(terminal, 0);
            }
        }
        self.dock.publish(&mut self.workspace);
    }

    /// Types `data` into the dock's shell, starting a new shell first when the last one ended.
    pub fn terminal_input(&mut self, data: Vec<u8>) {
        self.change_dock(|dock| dock.input(data));
    }

    /// Makes every shell `columns` characters wide and `rows` lines tall, as
    /// the dock now is, and returns whether that changed anything.
    pub fn resize_terminal(&mut self, columns: u16, rows: u16) -> bool {
        let mut changed = false;
        self.change_dock(|dock| changed = dock.resize(columns, rows));
        changed
    }

    /// Ends every shell this link started and forgets them, for a link that
    /// only looked, such as a snapshot's.
    pub fn close_terminals(&mut self) {
        self.change_dock(Dock::close_all);
    }

    /// Starts another shell in the dock's folder and shows it.
    pub fn new_terminal(&mut self) {
        self.change_dock(Dock::new_shell);
    }

    /// Shows the shell whose tab is kept as `key`.
    pub fn pick_terminal(&mut self, key: u64) {
        self.change_dock(|dock| dock.pick(key));
    }

    /// Ends the shell whose tab is kept as `key` and drops the tab.
    pub fn close_terminal(&mut self, key: u64) {
        self.change_dock(|dock| dock.close(key));
    }

    /// Scrolls the shown shell `lines` back, when positive, or forward.
    pub fn scroll_terminal(&mut self, lines: i32) {
        self.change_dock(|dock| dock.scroll(lines));
    }

    /// Marks text of the shown shell from `anchor` to `head`, in lines of its window, by cells, words or lines as `clicks` says.
    pub fn select_terminal(&mut self, anchor: (usize, usize), head: (usize, usize), clicks: u32) {
        self.change_dock(|dock| dock.select(anchor, head, clicks));
    }

    /// The text marked in the shown shell.
    pub fn terminal_selection(&mut self) -> Option<String> {
        self.dock.selected_text()
    }

    /// Types `text` into the shown shell as a paste.
    pub fn paste_terminal(&mut self, text: &str) {
        self.change_dock(|dock| dock.paste(text));
    }

    /// Marks everything the shown shell keeps.
    pub fn select_all_terminal(&mut self) {
        self.change_dock(Dock::select_all);
    }

    /// Clears the shown shell's window and what it keeps above it.
    pub fn clear_terminal(&mut self) {
        self.change_dock(Dock::clear);
    }

    /// Runs the task at `index` of the Tasks tab in a new shell tab, and shows the terminal.
    pub fn run_task(&mut self, index: usize) {
        self.change_dock(|dock| dock.run_task(index));
        self.workspace.dock_tab = crate::model::DockTab::Terminal;
    }

    /// Asks again for the tasks and debug scenarios of the dock's folder.
    pub fn refresh_dock(&mut self) {
        self.change_dock(Dock::refresh);
    }

    /// Starts debugging the dock's folder as the scenario at `index`.
    pub fn debug_start(&mut self, index: usize) {
        self.change_dock(|dock| dock.debug_start(index));
    }

    /// Does `command` to the debugging session the dock follows.
    pub fn debug_command(&mut self, command: DebugCommand) {
        self.change_dock(|dock| dock.debug_command(command));
    }

    /// Sets a breakpoint on `line` of `path`, counted from zero, or takes the one there away.
    pub fn toggle_breakpoint(&mut self, path: String, line: usize) {
        self.change_dock(|dock| dock.toggle_breakpoint(path, line));
    }

    /// Sends `request`, remembering it was for `ask`.
    fn ask(&mut self, request: Request, ask: Ask) {
        if let Some((host, request)) = self.routes.outgoing(&request) {
            self.ask_remote(&host, request, ask);
            return;
        }
        if let Some(client) = &self.client {
            let id = client.request(request);
            self.asks.insert(id, ask);
        }
    }

    /// Handles one message from the server.
    fn handle(&mut self, message: Incoming) {
        match message {
            Incoming::Connected => {
                self.workspace.status = None;
                self.forget_watches(None);
                self.web.forget_announcement();
                self.ask(Request::Projects, Ask::Projects);
                self.ask(Request::Sessions, Ask::Sessions);
                self.send(Request::MachineSettings);
                if self.settings.window.general.check_updates {
                    self.send(release_request(self.settings.window.general.channel));
                }
                self.change_dock(Dock::connected);
            }
            Incoming::Disconnected(reason) => self.workspace.status = Some(reason),
            Incoming::Response { id, response } => {
                let ask = self.asks.remove(&id).unwrap_or(Ask::Nothing);
                self.answer(ask, response);
            }
            Incoming::SessionAdded(info) => {
                self.follow(&info);
                let id = info.session;
                self.sessions.push(info);
                if self.watched == Some(id) {
                    self.show_session(id);
                } else {
                    self.rebuild();
                }
            }
            Incoming::ChatContinued {
                started, failure, ..
            } => {
                if let Some(info) = started {
                    let id = info.session;
                    self.follow(&info);
                    self.sessions.push(info);
                    self.reopen_session(id);
                } else if let Some(failure) = failure {
                    self.workspace.status = Some(failure);
                }
            }
            Incoming::SessionClosed(session) => {
                self.sessions.retain(|info| info.session != session);
                self.rebuild();
            }
            Incoming::ProjectsChanged(paths) => self.set_projects(paths),
            Incoming::FolderChanged(directory) => {
                self.reload_open_files(&directory);
                self.git_folder_changed(&directory);
                self.tree_folder_changed(&directory);
            }
            Incoming::ExtensionsChanged(extensions) => {
                self.workspace.extensions = Some(listed(&extensions));
                let changed = self.settings.extensions != extensions;
                self.settings.extensions = extensions;
                self.settings.extensions_loaded = true;
                if changed {
                    self.send(Request::ExtensionAssets);
                }
            }
            Incoming::Event {
                session,
                seq,
                event,
            } => self.event(session, seq, event),
            Incoming::TerminalOutput { terminal, data } => {
                self.change_dock(|dock| dock.output(terminal, &data))
            }
            Incoming::TerminalExited(terminal) => self.change_dock(|dock| dock.exited(terminal)),
            Incoming::Diagnostics {
                directory,
                path,
                diagnostics,
                ..
            } => self.take_diagnostics(directory, path, diagnostics),
            Incoming::Debug {
                debug,
                view,
                events,
            } => self.change_dock(|dock| dock.debug_update(debug, view, events)),
            Incoming::UpdateProgress { received, total } => {
                if let Some(offer) = &mut self.workspace.update
                    && offer.run.busy()
                {
                    offer.run = UpdateRun::Downloading { received, total };
                }
            }
            Incoming::SignInCode { agent, url, code } => {
                self.workspace.sign_in = Some(SignInCode { agent, url, code });
            }
            Incoming::BrowserCall {
                call,
                session,
                tool,
                arguments,
            } => self.browser_call(call, session, &tool, &arguments),
            Incoming::TerminalServing { url, .. } => self.served(None, &url),
            Incoming::SessionServing { session, url } => self.served(Some(session), &url),
        }
    }

    /// Handles the `response` to a request made for `ask`.
    fn answer(&mut self, ask: Ask, response: Response) {
        match (ask, response) {
            (Ask::Git(ask), response) => self.git_answer(ask, response),
            (ask @ (Ask::SessionSlots(_) | Ask::SessionAllowance(_)), response) => {
                self.composer_answer(ask, response)
            }
            (ask @ (Ask::Identity(_) | Ask::Discover(_) | Ask::StartBranches(_)), response) => {
                self.start_answer(ask, response)
            }
            (
                ask @ (Ask::Files(_) | Ask::Entry | Ask::Search(_) | Ask::Quick(_) | Ask::Branches),
                response,
            ) => self.answer_browse(ask, response),
            (Ask::PullRequest, Response::PullRequest { request, .. })
            | (Ask::PullRequest, Response::PullRequestActed { request, .. }) => {
                self.show_pull_request(request);
            }
            (Ask::Projects, Response::Projects { paths, saved }) => {
                self.note_saved(saved);
                self.set_projects(paths);
            }
            (Ask::Folders(host), response) => self.answer_folders(host, response),
            (Ask::Cloned, response) => self.answer_clone(response),
            (Ask::Sessions, Response::Sessions { sessions }) => {
                for info in &sessions {
                    self.follow(info);
                }
                self.sessions = sessions;
                self.rebuild();
                self.open_pending();
            }
            (
                Ask::Start,
                Response::Started {
                    session, directory, ..
                },
            ) => {
                let directory = match self.routes.remote(session) {
                    Some((host, _)) => routes::address_on(host, &directory),
                    None => directory,
                };
                self.hand_over(session, &directory);
                self.threads.insert(session, Thread::default());
                self.watch(session);
                self.navigate(Page::Session(session));
            }
            (Ask::Start, Response::Failed { message }) => self.workspace.status = Some(message),
            (Ask::Fork, response) => self.answer_fork(response),
            (Ask::Close(session), response) => self.answer_close(session, response),
            (Ask::Submodules(path), Response::Submodules { paths }) => {
                self.submodules.insert(path, paths);
                self.rebuild();
            }
            (Ask::Dock(ask), response) => self.change_dock(|dock| dock.answer(ask, response)),
            (Ask::Extensions, Response::Extensions { extensions }) => {
                self.workspace.extensions = Some(listed(&extensions));
                let changed = self.settings.extensions != extensions;
                self.settings.extensions = extensions;
                self.settings.extensions_loaded = true;
                if changed {
                    self.send(Request::ExtensionAssets);
                }
            }
            (Ask::ExtensionAssets, Response::ExtensionAssets(assets)) => {
                self.settings.assets = assets;
            }
            (Ask::Log(path), Response::Log { commits })
                if self.workspace.page == Page::Overview(path.clone()) =>
            {
                self.workspace.commits = commits
                    .into_iter()
                    .map(|commit| Commit {
                        id: commit.id.chars().take(SHORT_ID).collect(),
                        summary: commit.summary,
                    })
                    .collect();
            }
            (Ask::Code(code), response) => self.answer_code(code, response),
            (Ask::Server(purpose), response) => self.answer_server(purpose, response),
            (Ask::Excerpt(excerpt), response) => self.answer_excerpt(excerpt, response),
            (Ask::Listing(choosing), response) => self.answer_listing(choosing, response),
            (ask, response) => self.answer_settings(ask, response),
        }
    }

    /// Handles the `response` to a request the settings window made for `ask`.
    fn answer_settings(&mut self, ask: Ask, response: Response) {
        let settings = &mut self.settings;
        match (ask, response) {
            (Ask::MachineSettings, Response::MachineSettings { settings: machine }) => {
                settings.machine = machine;
                settings.machine_loaded = true;
            }
            (Ask::McpServers, Response::McpServers { servers }) => {
                settings.mcp_servers = servers;
                settings.mcp_loaded = true;
            }
            (Ask::ModelServers, Response::ModelServers { servers }) => {
                settings.model_servers = servers;
                settings.model_servers_loaded = true;
            }
            (Ask::ModelSlots, Response::ModelSlots { slots, presets }) => {
                settings.slots = slots;
                settings.presets = presets;
                settings.slots_loaded = true;
            }
            (
                Ask::Library,
                Response::Library {
                    items, withheld, ..
                },
            ) => {
                settings.library = items;
                settings.withheld = withheld;
                settings.library_loaded = true;
            }
            (Ask::ToolSets, Response::ToolSets { sets, withheld, .. }) => {
                settings.tool_sets = sets;
                settings.withheld = withheld;
            }
            (
                Ask::LatestRelease,
                Response::LatestRelease {
                    version,
                    url,
                    published,
                    body,
                    ..
                },
            ) => {
                settings.take_release(version.clone(), url.clone());
                self.workspace.update = match (&settings.release, version) {
                    (Some(Release::Newer { name, url }), Some(version)) => Some(UpdateOffer {
                        name: name.clone(),
                        version,
                        url: url.clone(),
                        published: published
                            .map(|date| date.split('T').next().unwrap_or(&date).to_owned()),
                        notes: body.filter(|notes| !notes.trim().is_empty()),
                        ..UpdateOffer::default()
                    }),
                    _ => None,
                };
            }
            (Ask::LatestRelease, _) => settings.release = Some(Release::Unknown),
            (Ask::Form, response) => {
                for request in settings.take_form_answer(response) {
                    self.send(request);
                }
            }
            (Ask::Update, Response::UpdateStaged { path }) => {
                if let Some(offer) = &mut self.workspace.update {
                    offer.run = UpdateRun::Installing;
                }
                self.staged = Some(path);
            }
            (Ask::Update, Response::Failed { message }) => self.update_failed(message),
            (Ask::Update, _) => {
                if let Some(offer) = &mut self.workspace.update {
                    offer.run = UpdateRun::Idle;
                }
            }
            (_, Response::Failed { message }) => self.workspace.status = Some(message),
            _ => {}
        }
    }

    /// Takes `paths` as the projects and asks for the submodules of each.
    fn set_projects(&mut self, paths: Vec<String>) {
        for path in &paths {
            if !self.submodules.contains_key(path) && pm_protocol::host_of(path).is_none() {
                self.ask(
                    Request::Submodules {
                        directory: expand_home(path),
                    },
                    Ask::Submodules(path.clone()),
                );
            }
        }
        self.paths = paths;
        self.ask_remote_sessions();
        self.rebuild();
    }

    /// Builds the sidebar's projects again from the paths, submodules and sessions.
    fn rebuild(&mut self) {
        let now = now_ms();
        let open_id = self.watched;
        let threads = &self.threads;
        let reopened = &self.reopened;
        let mut history: Vec<&SessionInfo> = self.sessions.iter().collect();
        history.sort_by_key(|info| std::cmp::Reverse(info.updated));
        self.workspace.history = history
            .iter()
            .map(|info| PastSession::of(info, now))
            .collect();
        let mut sessions: Vec<&SessionInfo> = self
            .sessions
            .iter()
            .filter(|info| (!info.closed || reopened.contains(&info.session)) && !info.chat)
            .collect();
        sessions.sort_by_key(|info| std::cmp::Reverse(info.updated));
        let folded = &self.folded;
        let entry = |info: &SessionInfo| Session {
            id: info.session,
            title: info.title.clone(),
            age: age(now.saturating_sub(info.updated)),
            status: threads
                .get(&info.session)
                .map(|thread| thread.status(info.running))
                .unwrap_or_default(),
            branch: info.branch.clone().unwrap_or_default(),
            directory: info.directory.clone(),
        };
        self.workspace.projects = self
            .paths
            .iter()
            .map(|path| {
                let children = self
                    .submodules
                    .get(path)
                    .into_iter()
                    .flatten()
                    .map(|submodule| Project {
                        name: submodule.rsplit('/').next().unwrap_or(submodule).to_owned(),
                        path: format!("{path}/{submodule}"),
                        children: Vec::new(),
                        sessions: sessions
                            .iter()
                            .filter(|info| {
                                same_folder(&info.repository, path)
                                    && info.submodule.as_deref() == Some(submodule.as_str())
                            })
                            .map(|info| entry(info))
                            .collect(),
                        collapsed: folded.contains(&format!("{path}/{submodule}")),
                        reach: None,
                    })
                    .collect();
                Project {
                    name: path
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or(path)
                        .to_owned(),
                    path: path.clone(),
                    children,
                    sessions: sessions
                        .iter()
                        .filter(|info| {
                            same_folder(&info.repository, path) && info.submodule.is_none()
                        })
                        .map(|info| entry(info))
                        .collect(),
                    collapsed: folded.contains(path),
                    reach: None,
                }
            })
            .collect();
        let mut chats: Vec<&SessionInfo> = self
            .sessions
            .iter()
            .filter(|info| info.chat && (!info.closed || reopened.contains(&info.session)))
            .collect();
        chats.sort_by_key(|info| std::cmp::Reverse(info.updated));
        self.workspace.chats = chats.into_iter().map(entry).collect();
        self.place_remote_projects(now);
        self.show_sample();
        if let Some(open_id) = open_id {
            self.workspace.open = find_session(&self.workspace.projects, open_id, Vec::new());
        }
    }

    /// Takes the titles of sessions whose agent finished a turn since the last call.
    pub fn take_finished(&mut self) -> Vec<String> {
        std::mem::take(&mut self.finished)
    }

    /// Starts following the events of the session `info` describes, once.
    fn follow(&mut self, info: &SessionInfo) {
        if self.threads.contains_key(&info.session) {
            return;
        }
        self.threads.insert(info.session, Thread::default());
        self.watch(info.session);
    }

    /// Follows the events of the session the window numbers `id`, from its first, on the server of the machine it runs on.
    fn watch(&mut self, id: u64) {
        match self.routes.remote(id) {
            Some((host, number)) => {
                let host = host.to_owned();
                self.remotes.watch(&host, number);
            }
            None => {
                if let Some(client) = &self.client {
                    client.watch(id, 0);
                }
            }
        }
    }

    /// What the server of its machine says of the session the window numbers `id`, with the window's number and, for another machine's, its folders as addresses on that machine.
    fn session_info(&self, id: u64) -> Option<SessionInfo> {
        let Some((host, number)) = self.routes.remote(id) else {
            return self
                .sessions
                .iter()
                .find(|info| info.session == id)
                .cloned();
        };
        let mut info = self
            .remote_sessions
            .get(host)?
            .iter()
            .find(|info| info.session == number)?
            .clone();
        info.session = id;
        for folder in [&mut info.directory, &mut info.repository] {
            if !folder.is_empty() {
                *folder = routes::address_on(host, folder);
            }
        }
        Some(info)
    }

    /// Handles event number `seq` of `session`.
    fn event(&mut self, session: u64, seq: u64, event: Event) {
        let info = self.session_info(session);
        let replayed = info.as_ref().is_some_and(|info| seq <= info.last_seq);
        let open = self.watched == Some(session);
        let local = self.routes.remote(session).is_none();
        let seen = self
            .workspace
            .read_through
            .get(&session)
            .filter(|_| local)
            .map_or(replayed, |read| seq <= *read);
        let finishing = matches!(event, Event::Finished(_));
        let thread = self.threads.entry(session).or_default();
        if !thread.take(seq, event) {
            return;
        }
        if seen || open {
            thread.mark_read();
        }
        if open && local {
            self.workspace.read_through.insert(session, seq);
        }
        if finishing && !replayed {
            let title = info
                .map(|info| thread.title.clone().unwrap_or(info.title))
                .unwrap_or_default();
            self.finished.push(title);
        }
        if open {
            show_thread(&mut self.workspace, thread);
        }
        self.rebuild();
    }
}

/// How many of a project's latest commits its overview lists.
const COMMITS_SHOWN: usize = 8;

/// How many characters of a commit's name the overview shows.
const SHORT_ID: usize = 7;

/// Puts what `thread` knows of the open session into `workspace`.
fn show_thread(workspace: &mut Workspace, thread: &Thread) {
    if let Some(title) = &thread.title {
        workspace.title = title.clone();
    }
    workspace.model = thread.model.clone().unwrap_or_default();
    workspace.permissions = thread.mode.clone().unwrap_or_default();
    workspace.conversation = thread.messages.clone();
    let kept = std::mem::take(&mut workspace.extras);
    workspace.extras = Extras {
        pull_request: kept.pull_request,
        chat: kept.chat,
        prefill: kept.prefill,
        composer: ComposerState {
            files: kept.composer.files,
            elements: kept.composer.elements,
            captures: kept.composer.captures,
            menu: kept.composer.menu,
            asked: kept.composer.asked,
            ..thread.extras.composer.clone()
        },
        ..thread.extras.clone()
    };
}

/// The session at index path `at` among `projects`.
pub fn session_at<'a>(projects: &'a [Project], at: &[usize]) -> Option<&'a Session> {
    let (last, parents) = at.split_last()?;
    let (first, nested) = parents.split_first()?;
    let mut project = projects.get(*first)?;
    for index in nested {
        project = project.children.get(*index)?;
    }
    project.sessions.get(*last)
}

/// The index path of the session numbered `id` among `projects`, under `prefix`.
fn find_session(projects: &[Project], id: u64, prefix: Vec<usize>) -> Option<Vec<usize>> {
    for (index, project) in projects.iter().enumerate() {
        let mut here = prefix.clone();
        here.push(index);
        if let Some(position) = project.sessions.iter().position(|session| session.id == id) {
            let mut found = here.clone();
            found.push(position);
            return Some(found);
        }
        if let Some(found) = find_session(&project.children, id, here) {
            return Some(found);
        }
    }
    None
}

/// Whether two project addresses name the same folder, with `~` expanded.
fn same_folder(left: &str, right: &str) -> bool {
    expand_home(left).trim_end_matches('/') == expand_home(right).trim_end_matches('/')
}

/// `path` with a leading `~/` replaced by the home folder.
fn expand_home(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest).to_string_lossy().into_owned(),
        _ => path.to_owned(),
    }
}

/// Milliseconds since the epoch.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

/// How long `elapsed` milliseconds is, as the sidebar says it: `just now`, `12m ago`, `4d ago`.
pub fn age(elapsed: u64) -> String {
    let minutes = elapsed / 60_000;
    match minutes {
        0 => "just now".to_owned(),
        1..60 => format!("{minutes}m ago"),
        60..1440 => format!("{}h ago", minutes / 60),
        _ => format!("{}d ago", minutes / 1440),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ages read in the largest whole unit.
    #[test]
    fn ages_read_in_whole_units() {
        assert_eq!(age(10_000), "just now");
        assert_eq!(age(12 * 60_000), "12m ago");
        assert_eq!(age(3 * 3_600_000), "3h ago");
        assert_eq!(age(4 * 86_400_000), "4d ago");
    }

    /// A session is found by its index path through nested projects.
    #[test]
    fn finds_sessions_by_index_path() {
        let projects = sample_workspace().projects;
        assert_eq!(
            session_at(&projects, &[2, 2, 0]).map(|session| session.id),
            Some(7)
        );
        assert_eq!(find_session(&projects, 9, Vec::new()), Some(vec![3, 0]));
    }
}
