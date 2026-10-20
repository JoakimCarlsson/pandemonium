//! The work behind the screen: asking the servers to run the agents and the git operations, and taking in what they answer.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};

use pm_acp::{Discovery, Event};
use pm_client::{Client, Incoming};
use pm_components::adder::{Found, Kind};
use pm_components::figures::Drawing;
use pm_components::git_controls::GRAPH_LIMIT;
use pm_components::state::SESSION_BASE;
use pm_components::tree_actions::EntryChange;
use pm_input::FocusId;
use pm_pages::{Outgoing, Screen, WorkspaceState};
use pm_protocol::{Attachment, Login, Request, Response, Target};
use pm_renderer::Picture;
use pm_state::State;

use crate::view::View;

mod code;
mod debug;
mod machine;
pub(crate) mod picture;
mod tasks;
mod watch;

/// The most files listed for the list behind an `@`.
const FILE_LIMIT: usize = 20_000;

/// What failing to carry out `change` is called.
fn entry_failure(change: &EntryChange) -> &'static str {
    match change {
        EntryChange::Create { folder: true, .. } => "make the folder",
        EntryChange::Create { folder: false, .. } => "make the file",
        EntryChange::Rename { .. } => "rename it",
        EntryChange::Copy { .. } => "copy it",
        EntryChange::Remove { .. } => "delete it",
    }
}

/// The request that carries out `change` in `directory`.
fn entry_request(change: EntryChange, directory: String) -> Request {
    match change {
        EntryChange::Create { path, folder } => Request::CreateEntry {
            directory,
            path,
            folder,
        },
        EntryChange::Rename { from, to } => Request::RenameEntry {
            directory,
            from,
            to,
        },
        EntryChange::Copy { from, into } => Request::CopyEntry {
            directory,
            from,
            into,
        },
        EntryChange::Remove { path } => Request::RemoveEntry { directory, path },
    }
}

/// What the window opens with, as the server on this machine keeps it.
pub struct Bootstrap {
    /// The projects the reader added, or nothing when they were never saved.
    pub projects: Option<Vec<String>>,
    /// What the reader chose for each agent's settings.
    pub choices: Vec<(String, String, String)>,
    /// The named logins.
    pub logins: Vec<(String, String, String)>,
    /// The login in use for each agent.
    pub in_use: Vec<(String, String)>,
    /// The agents the server can start.
    pub agents: Vec<pm_protocol::AgentServer>,
}

/// How long the window waits for the server on this machine to say what it keeps.
const BOOTSTRAP_WAIT: std::time::Duration = std::time::Duration::from_secs(4);

/// A machine the servers run on: nothing for this one, or how a project's address names another.
type Host = Option<String>;

/// What a thread that did some of the work reports back.
enum Job {
    /// The picture at this path was decoded, when it is one.
    Previewed(String, Option<Picture>),
    /// The picture file of the session at this position, at this path, and whether it was read from the last commit, was decoded, or could not be.
    Pictured(usize, String, bool, Result<Picture, String>),
    /// The text of the vector picture of the session at this position, at this path, and whether it was read from the last commit, was drawn, or could not be.
    Drawn(usize, String, bool, Result<Picture, String>),
    /// The figure kept under this key was drawn or decoded, or could not be.
    Figure(u64, Result<Drawing, String>),
    /// The server on this machine was found or reached, or could not be.
    Server(Host, Result<Target, String>),
    /// The server of this machine is being brought up, and has got this far.
    Starting(Host, String),
    /// The grammar of the language with this folder name was compiled, or could not be.
    Grammar(String, Result<(), String>),
}

/// What is to be done with the answer to a request.
#[derive(Clone, Debug)]
enum Pending {
    /// Nothing: the answer only matters if it says the request failed.
    Nothing,
    /// What the server on this machine keeps for the work the servers do.
    MachineSettings,
    /// The newest release on a channel.
    LatestRelease,
    /// The agents the reader added by hand.
    AgentServers,
    /// The tool servers every agent is started with.
    McpServers,
    /// The agents the public registry offers.
    RegistryAgents,
    /// The tool servers the public registry offered for this search.
    Tools(String),
    /// The agent with this id told what it lets the reader change.
    Discover(Host, String),
    /// The agent at this position was signed in, or could not be.
    SignIn(usize),
    /// The skills of an agent in a project.
    Skills(usize, usize),
    /// The sessions the server of this machine holds.
    Sessions(Host),
    /// The files listed for this key.
    Files(String),
    /// The branches of the project at this position.
    Branches(usize),
    /// Who an agent is signed in as, and whether that names the login in use.
    Identity(usize, bool),
    /// The session at this position was started.
    Start(usize),
    /// The folder of the session at this position was looked at.
    Inspect(usize),
    /// Files of the session at this position were staged or unstaged.
    Stage(usize),
    /// A commit was made in the folder of the session at this position.
    Commit(usize, bool),
    /// The remote answered the session at this position, which asked it to fetch, pull, push or sync.
    Remote(usize),
    /// The commits of the repository of the session at this position.
    Log(usize),
    /// The stashes of the repository of the session at this position.
    Stashes(usize),
    /// Something was done to the files or branches of the folder of the session at this position, with what failing at it is called.
    Git(usize, &'static str),
    /// A file or folder of the folder of the session at this position was made, renamed, copied or removed, with what failing at it is called.
    Entry(usize, &'static str),
    /// A file of the folder of the session at this position was written, at this path.
    Write(usize, String),
    /// The branches of the folder of the session at this position.
    SessionBranches(usize),
    /// The file of the session at this position, at this path, and whether it was read from the last commit.
    ReadFile(usize, String, bool),
    /// The bytes of the picture file of the session at this position, at this path, and whether they were read from the last commit.
    ReadPicture(usize, String, bool),
    /// The bytes of the figure of markdown kept under this key.
    Figure(u64),
    /// A session was branched off the session at this position.
    Fork(usize),
    /// The session at this position went back, with what its failing is called.
    Back(usize, &'static str),
    /// The shell of the tab with this name was started, or could not be.
    OpenTerminal(u64),
    /// The shells the server of this machine holds.
    Terminals(Host),
    /// The GitHub accounts the panel for adding a project is waiting to list.
    Owners,
    /// The GitHub repositories of this account the panel for adding a project is waiting to list.
    Repositories(String),
    /// The folders inside this folder, which the panel for adding a project waits to list.
    Folders(String),
    /// A repository the panel for adding a project asked to have cloned.
    Added,
    /// The grammar of a language, asked for by this name.
    Grammar(String),
    /// The file of the session at this position, at this path, was opened for its language server.
    Opened(usize, String),
    /// What could be written in the file of the session at this position, at this path, as asked at this version.
    Completions(usize, String, i32),
    /// What the language server says about this place of the file of the session at this position, at this path.
    Hovered(usize, String, pm_protocol::Position),
    /// The extensions the server on this machine can install.
    Extensions,
    /// What the extension package in this folder carries and asks to be allowed.
    ExtensionPreview(String),
    /// What the extensions of the server of this machine bring that the window uses itself.
    ExtensionAssets(Host),
    /// What a command an extension brings came to, to tell in this session.
    ExtensionCommand(Option<usize>),
    /// The places looked for from a place in a file of the session at this position.
    Definitions(usize, pm_protocol::Seek),
    /// Who last changed this line of the file of the session at this position, at this path.
    Blame(usize, String, usize),
    /// What is wrong with the files of the project of the session at this position.
    Problems(usize),
    /// The symbols of the project of the session at this position matching this query.
    Symbols(usize, String),
    /// The file of the session at this position, at this path, as the last commit holds it.
    Base(usize, String),
    /// Where the thing at a place of the file of the session at this position, at this path and version, is used.
    Highlights(usize, String, i32),
    /// The signature of the call a place of the file of the session at this position, at this path and version, is in.
    Signature(usize, String, i32),
    /// The hits of a search of the project of the session at this position for a text, matching case or not, as a regular expression or not.
    ProjectSearch(usize, String, bool, bool),
    /// What each name of the file of the session at this position, at this path and version, is.
    SemanticTokens(usize, String, i32),
    /// What to show inside the lines of the file of the session at this position, at this path and version.
    InlayHints(usize, String, i32),
    /// Which names of the file of the session at this position, at this path and version, to show the values of while a program is paused.
    InlineValues(usize, String, i32),
    /// What to show at the end of the lines of the file of the session at this position, at this path and version.
    CodeLenses(usize, String, i32),
    /// What pressing a lens of a file of the session at this position came to.
    LensPressed(usize),
    /// The edits that rename something across the project of the session at this position.
    Renamed(usize),
    /// The changes the language server offers for the file of the session at this position, at this path.
    CodeActions(usize, String),
    /// The edits a change the language server offered makes across the project of the session at this position.
    TookAction(usize),
    /// The web address of a file or a line, opened in the browser when true and copied otherwise.
    Permalink(bool),
    /// The commits that changed the file of the session at this position, at this path.
    FileHistory(usize, String),
    /// A file of the session at this position as a commit left it, shown under this name.
    Revision(usize, String),
    /// The states the file of the session at this position, at this path, was saved in.
    LocalHistory(usize, String),
    /// A state the file of the session at this position, at this path, was saved in, saved this long ago.
    Version(usize, String, String),
    /// The versions of the conflicted file of the session at this position, at this path.
    Conflict(usize, String),
    /// The files a commit or a branch changed in the folder of the session at this position, shown under this name.
    Revisions(usize, String),
    /// The edits that format the file of the session at this position, at this path and version, and whether it is written afterwards.
    Formatted(usize, String, i32, bool),
    /// What the folder of the session at this position can be debugged as.
    DebugScenarios(usize),
    /// The programs a debugger can attach to on the machine of the session at this position.
    DebugProcesses(usize),
    /// The program debugged for the session at this position was started.
    DebugStarted(usize),
    /// What the folder of the session at this position can be built, tested and run with.
    Tasks(usize),
    /// The toolchains installed on the machine of the session at this position.
    Toolchains(usize),
    /// The programs the server of this machine debugs.
    Debugs(Host),
    /// A program the server still debugs, picked up for the session at this position.
    DebugWatched(usize),
    /// The program with this number, which the window shows, watched again after the connection was made anew.
    DebugRewatched(u64),
    /// Which blocks of the file of the session at this position, at this path and version, can be folded.
    FoldingRanges(usize, String, i32),
    /// Which stretches of the file of the session at this position, at this path and version, are edited together with the one at a place.
    LinkedRanges(usize, String, i32),
}

impl Pending {
    /// Whether this is waiting on the server for something about a session's folder that the source control panel shows as under way.
    fn is_git(&self) -> bool {
        matches!(
            self,
            Self::Inspect(_)
                | Self::Log(_)
                | Self::Stashes(_)
                | Self::Stage(_)
                | Self::Commit(..)
                | Self::Git(..)
                | Self::Remote(_)
                | Self::SessionBranches(_)
        )
    }
}

/// Something done to a shell before the server said which shell it is, kept until it does.
#[derive(Clone, Debug)]
enum Held {
    /// Bytes typed.
    Input(Vec<u8>),
    /// The window's new size, as columns and rows.
    Resize(u16, u16),
    /// The tab was closed.
    Close,
}

/// Where a session works and what it is made of.
struct Placement {
    /// The machine it is on.
    host: Host,
    /// The folder its agent works in.
    directory: String,
    /// Which agent runs it, by position.
    agent: usize,
    /// The branch its worktree was made from, when it has one of its own.
    base: Option<String>,
    /// The folder of the project it belongs to.
    repository: String,
    /// What it is called.
    title: String,
}

/// The connection to the server of one machine, and what is waiting for its answers.
#[derive(Default)]
struct Link {
    /// The connection, once the server is reached.
    client: Option<Client>,
    /// Requests made before the server was reached.
    backlog: Vec<(Request, Pending)>,
    /// What to do with the answer to each request, by its id.
    waiting: HashMap<u64, Pending>,
}

/// The servers and the threads doing work for them.
pub struct Work {
    /// The link to the server of each machine that has been asked something.
    links: HashMap<Host, Link>,
    /// What every server is to work by, once the server on this machine said.
    machine: Option<pm_protocol::MachineSettings>,
    /// The tokens of the servers reached straight at their address or through a tunnel, by address.
    tokens: HashMap<String, String>,
    /// The number of the last event each session had when the window first listed it, by machine and number; the events up to it are history, not news.
    history: HashMap<(Host, u64), u64>,
    /// The folder each server was last asked to watch, by machine.
    watched: HashMap<Host, Option<String>>,
    /// The sessions, by position, whose folders were asked to be read while a read of them was on its way, so they are read once more when it lands.
    read_again: HashSet<usize>,
    /// The machine and the server's number of each session, by the session's position.
    numbers: HashMap<usize, (Host, u64)>,
    /// The session's position, by the machine and the server's number.
    positions: HashMap<(Host, u64), usize>,
    /// The sessions the window let go of with their project, whose events are not wanted until the project is added again.
    forgotten: HashSet<(Host, u64)>,
    /// What agents did before their session was known by position, each with its number.
    early: HashMap<(Host, u64), Vec<(u64, Event)>>,
    /// What the threads report.
    jobs: Receiver<Job>,
    /// Handed to each thread.
    reporter: Sender<Job>,
    /// Whether the agents on this machine have been asked what they let the reader change.
    discovered: bool,
    /// The ids of the agents the server lists, in the order the window lists them.
    agents: Vec<String>,
    /// The machine and the server's number of the shell of each tab.
    tabs: HashMap<u64, (Host, u64)>,
    /// The tab, by the machine and the server's number of its shell.
    shells: HashMap<(Host, u64), u64>,
    /// What was done to the tabs whose shells are not known yet, in order.
    held: HashMap<u64, Vec<Held>>,
    /// What shells printed before their tab was known, by machine and number.
    unclaimed: HashMap<(Host, u64), Vec<u8>>,
    /// Where the window was left, to be finished once the server that holds its session has listed its sessions.
    resume: Option<View>,
    /// How many threads have yet to report.
    pending: Cell<usize>,
    /// The grammars asked for of each machine, by the name they were asked by, so each is asked for once.
    grammars: HashSet<(Host, String)>,
    /// The colours diagrams are drawn in.
    palette: picture::Palette,
    /// What looking for a newer release last found, kept for a workspace that opens after it was answered.
    release: Option<pm_components::update::Update>,
}

impl Work {
    /// No servers reached yet: they are looked for when something is first asked of them.
    pub fn new() -> Self {
        let (reporter, jobs) = mpsc::channel();
        Self {
            links: HashMap::new(),
            machine: None,
            tokens: HashMap::new(),
            history: HashMap::new(),
            watched: HashMap::new(),
            read_again: HashSet::new(),
            numbers: HashMap::new(),
            positions: HashMap::new(),
            forgotten: HashSet::new(),
            early: HashMap::new(),
            tabs: HashMap::new(),
            shells: HashMap::new(),
            held: HashMap::new(),
            unclaimed: HashMap::new(),
            jobs,
            reporter,
            discovered: false,
            agents: pm_protocol::builtin_agents()
                .into_iter()
                .map(|agent| agent.id)
                .collect(),
            resume: None,
            pending: Cell::new(0),
            grammars: HashSet::new(),
            palette: picture::Palette::default(),
            release: None,
        }
    }

    /// Asks the server on this machine what the window opens with, starting the server first when it is not running.
    ///
    /// Nothing is returned when the server cannot be reached in time; the window then opens with what is saved on this machine.
    pub fn bootstrap(&mut self) -> Option<Bootstrap> {
        let target = pm_client::local().ok()?;
        let client = Client::connect(target);
        let projects_id = client.request(Request::Projects);
        let choices_id = client.request(Request::Choices);
        let accounts_id = client.request(Request::Accounts);
        let agents_id = client.request(Request::AgentServers);
        let mut projects = None;
        let mut choices = None;
        let mut accounts = None;
        let mut agents = None;
        let started = std::time::Instant::now();
        while started.elapsed() < BOOTSTRAP_WAIT
            && (projects.is_none() || choices.is_none() || accounts.is_none() || agents.is_none())
        {
            for item in client.incoming() {
                let Incoming::Response { id, response } = item else {
                    continue;
                };
                if id == projects_id
                    && let Response::Projects { paths, saved } = response
                {
                    projects = Some(saved.then_some(paths));
                } else if id == choices_id
                    && let Response::Choices { choices: held } = response
                {
                    choices = Some(held);
                } else if id == accounts_id
                    && let Response::Accounts { logins, in_use } = response
                {
                    accounts = Some((logins, in_use));
                } else if id == agents_id
                    && let Response::AgentServers { servers } = response
                {
                    agents = Some(servers);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let (projects, choices, (logins, in_use), agents) =
            (projects?, choices?, accounts?, agents?);
        self.agents = agents.iter().map(|agent| agent.id.clone()).collect();
        self.links.insert(
            None,
            Link {
                client: Some(client),
                ..Link::default()
            },
        );
        self.ask(&None, Request::Sessions, Pending::Sessions(None));
        self.ask(&None, Request::Extensions, Pending::Extensions);
        self.ask_machine_settings();
        Some(Bootstrap {
            projects,
            choices,
            logins: logins
                .into_iter()
                .map(|login| (login.agent, login.id, login.name))
                .collect(),
            in_use,
            agents,
        })
    }

    /// Has the window finish opening on `view` once the sessions it needs are listed.
    pub fn resume(&mut self, view: View) {
        self.resume = Some(view);
    }

    /// Whether the window has yet to finish opening where it was left, which is when what it shows must not be kept.
    pub fn resuming(&self) -> bool {
        self.resume.is_some()
    }

    /// The machine and the server's number of the session at `position`.
    pub fn server_number(&self, position: usize) -> Option<(Host, u64)> {
        self.numbers.get(&position).cloned()
    }

    /// Has the server on this machine keep `paths` as the projects the reader added.
    pub fn save_projects(&mut self, paths: Vec<String>) {
        self.ask(&None, Request::SaveProjects { paths }, Pending::Nothing);
    }

    /// Has the server on this machine keep `choices` as what the reader chose for each agent's settings.
    pub fn save_choices(&mut self, choices: Vec<(String, String, String)>) {
        self.ask(&None, Request::SaveChoices { choices }, Pending::Nothing);
    }

    /// Sends `request` to the server on `host`, or keeps it until that server is reached, and remembers what to do with the answer.
    fn ask(&mut self, host: &Host, request: Request, pending: Pending) {
        if !self.links.contains_key(host) {
            self.links.insert(host.clone(), Link::default());
            let host = host.clone();
            let tokens = self.tokens.clone();
            self.spawn(move |reporter| {
                let progress = |text: &str| {
                    let _ = reporter.send(Job::Starting(host.clone(), text.to_owned()));
                };
                let target = match pm_protocol::route_of(host.as_deref()) {
                    pm_protocol::Route::Local => pm_client::local(),
                    pm_protocol::Route::Direct(address) => Ok(Target {
                        address: address.to_owned(),
                        token: tokens.get(address).cloned().unwrap_or_default(),
                    }),
                    pm_protocol::Route::Ssh(destination) => pm_ssh::remote(destination),
                    pm_protocol::Route::Wsl(distro) => pm_transport::wsl(distro, &progress),
                    pm_protocol::Route::Container(folder) => {
                        pm_transport::container(&folder, &progress)
                    }
                    pm_protocol::Route::Tunnel(hostname) => pm_transport::tunnel(
                        hostname,
                        tokens.get(hostname).map_or("", String::as_str),
                    ),
                };
                let _ = reporter.send(Job::Server(host, target));
            });
        }
        let Some(link) = self.links.get_mut(host) else {
            return;
        };
        match &link.client {
            Some(client) => {
                let id = client.request(request);
                link.waiting.insert(id, pending);
            }
            None => link.backlog.push((request, pending)),
        }
    }

    /// Points the agents of the server on this machine at the folder of the login in use for each.
    pub fn set_accounts(
        &mut self,
        logins: &[(String, String, String)],
        in_use: &[(String, String)],
    ) {
        let logins = logins
            .iter()
            .map(|(agent, id, name)| Login {
                agent: agent.clone(),
                id: id.clone(),
                name: name.clone(),
            })
            .collect();
        self.ask(
            &None,
            Request::SetAccounts {
                logins,
                in_use: in_use.to_vec(),
            },
            Pending::Nothing,
        );
    }

    /// Asks each agent on this machine what it lets the reader change, once the workspace is showing.
    pub fn discover(&mut self, model: &State<Screen>) {
        if self.discovered || !model.get().is_workspace() {
            return;
        }
        self.discovered = true;
        self.ask(&None, Request::AgentServers, Pending::AgentServers);
        for agent in self.agents.clone() {
            self.ask(
                &None,
                Request::Discover {
                    agent: agent.clone(),
                },
                Pending::Discover(None, agent),
            );
        }
    }

    /// The files at `paths`, read whole, for a prompt.
    fn attachments(paths: &[String]) -> Vec<Attachment> {
        paths
            .iter()
            .filter_map(|path| {
                let name = std::path::Path::new(path)
                    .file_name()?
                    .to_string_lossy()
                    .into_owned();
                Some(Attachment {
                    name,
                    data: std::fs::read(path).ok()?,
                })
            })
            .collect()
    }

    /// Takes `ids` as the agents the server lists, and asks the ones this window had not heard of what they let the reader change, once the others were asked.
    fn listed(&mut self, ids: Vec<String>) {
        let new: Vec<String> = ids
            .iter()
            .filter(|id| !self.agents.contains(id))
            .cloned()
            .collect();
        self.agents = ids;
        if !self.discovered {
            return;
        }
        for agent in new {
            self.ask(
                &None,
                Request::Discover {
                    agent: agent.clone(),
                },
                Pending::Discover(None, agent),
            );
        }
    }

    /// The agent at `position`, by the id the servers know it by.
    fn agent_id(&self, position: usize) -> Option<String> {
        self.agents.get(position).cloned()
    }

    /// Asks the server that runs the session at `session` to do `request`, when that session is running there.
    fn ask_session(
        &mut self,
        session: usize,
        build: impl FnOnce(u64) -> Request,
        pending: Pending,
    ) {
        if let Some((host, number)) = self.numbers.get(&session).cloned() {
            self.ask(&host, build(number), pending);
        }
    }

    /// Carries out what the reader asked of the agents and the folders since the last look, and returns what to copy.
    pub fn deliver(&mut self, model: &State<Screen>) -> Vec<String> {
        let mut outbox = Vec::new();
        model.update(|screen| outbox = screen.take_outbox());
        let mut copies = Vec::new();
        for outgoing in outbox {
            match outgoing {
                Outgoing::Reindexed(moved) => self.reindex(&moved),
                Outgoing::OpenTerminal {
                    session,
                    folder,
                    tab,
                    columns,
                    rows,
                } => {
                    let Some(placement) = Self::session_of(model, session) else {
                        continue;
                    };
                    let root = if placement.directory.is_empty() {
                        placement.repository
                    } else {
                        placement.directory
                    };
                    let directory = if folder.is_empty() {
                        root
                    } else {
                        format!("{}/{folder}", root.trim_end_matches(['/', '\\']))
                    };
                    self.held.insert(tab, Vec::new());
                    self.ask(
                        &placement.host,
                        Request::OpenTerminal {
                            directory,
                            columns,
                            rows,
                        },
                        Pending::OpenTerminal(tab),
                    );
                }
                Outgoing::TerminalInput { tab, data } => {
                    self.ask_terminal(tab, Held::Input(data));
                }
                Outgoing::ResizeTerminal { tab, columns, rows } => {
                    self.ask_terminal(tab, Held::Resize(columns, rows));
                }
                Outgoing::CloseTerminal { tab } => self.ask_terminal(tab, Held::Close),
                Outgoing::Rename { session, title } => self.ask_session(
                    session,
                    |number| Request::RenameSession {
                        session: number,
                        title,
                    },
                    Pending::Nothing,
                ),
                Outgoing::OpenFolder { session } => {
                    if let Some(placement) = Self::session_of(model, session)
                        && placement.host.is_none()
                    {
                        show_in_file_manager(&placement.directory);
                    }
                }
                Outgoing::Close { session, discard } => self.ask_session(
                    session,
                    |number| Request::CloseSession {
                        session: number,
                        discard,
                    },
                    Pending::Nothing,
                ),
                Outgoing::Start {
                    session,
                    text,
                    files,
                    settings,
                    base,
                } => {
                    let Some(placement) = Self::session_of(model, session) else {
                        continue;
                    };
                    let Some(agent) = self.agent_id(placement.agent) else {
                        continue;
                    };
                    self.ask(
                        &placement.host,
                        Request::StartSession {
                            agent,
                            repository: placement.repository,
                            directory: placement.directory,
                            title: placement.title,
                            base,
                            settings,
                            text,
                            attachments: Self::attachments(&files),
                        },
                        Pending::Start(session),
                    );
                }
                Outgoing::Prompt {
                    session,
                    text,
                    files,
                    now,
                } => {
                    let attachments = Self::attachments(&files);
                    self.ask_session(
                        session,
                        |number| Request::Prompt {
                            session: number,
                            text,
                            attachments,
                            now,
                        },
                        Pending::Nothing,
                    );
                }
                Outgoing::RemoveQueued { session, queued } => self.ask_session(
                    session,
                    |number| Request::RemoveQueued {
                        session: number,
                        queued,
                    },
                    Pending::Nothing,
                ),
                Outgoing::Hurry { session, queued } => self.ask_session(
                    session,
                    |number| Request::Hurry {
                        session: number,
                        queued,
                    },
                    Pending::Nothing,
                ),
                Outgoing::Choose {
                    session,
                    ticket,
                    option,
                } => self.ask_session(
                    session,
                    |number| Request::Choose {
                        session: number,
                        ticket,
                        option,
                    },
                    Pending::Nothing,
                ),
                Outgoing::Answer {
                    session,
                    ticket,
                    replies,
                } => self.ask_session(
                    session,
                    |number| Request::Answer {
                        session: number,
                        ticket,
                        replies,
                    },
                    Pending::Nothing,
                ),
                Outgoing::Rewind { session, turn } => self.ask_session(
                    session,
                    |number| Request::Rewind {
                        session: number,
                        turn,
                    },
                    Pending::Back(session, "Could not rewind the conversation"),
                ),
                Outgoing::Fork { session, turn } => self.ask_session(
                    session,
                    |number| Request::ForkSession {
                        session: number,
                        turn,
                    },
                    Pending::Fork(session),
                ),
                Outgoing::Restore { session, turn } => self.ask_session(
                    session,
                    |number| Request::Restore {
                        session: number,
                        turn,
                    },
                    Pending::Back(session, "Could not restore the files"),
                ),
                Outgoing::Cancel { session } => self.ask_session(
                    session,
                    |number| Request::Cancel { session: number },
                    Pending::Nothing,
                ),
                Outgoing::Set {
                    session,
                    setting,
                    value,
                } => self.ask_session(
                    session,
                    |number| Request::Set {
                        session: number,
                        setting,
                        value,
                    },
                    Pending::Nothing,
                ),
                Outgoing::Stage {
                    session,
                    paths,
                    stage,
                } => {
                    if let Some(placement) = Self::session_of(model, session) {
                        self.ask(
                            &placement.host,
                            Request::Stage {
                                directory: placement.directory,
                                paths,
                                stage,
                            },
                            Pending::Stage(session),
                        );
                    }
                }
                Outgoing::Commit {
                    session,
                    message,
                    tracked,
                    push,
                    amend,
                } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Commit {
                        directory,
                        message,
                        tracked,
                        amend,
                    },
                    Pending::Commit(session, push),
                ),
                Outgoing::Step {
                    session,
                    action,
                    message,
                } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Step {
                        directory,
                        action: action.to_owned(),
                        message,
                    },
                    Pending::Git(session, "continue"),
                ),
                Outgoing::Stash {
                    session,
                    action,
                    index,
                    message,
                } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Stash {
                        directory,
                        action: action.to_owned(),
                        index,
                        message,
                    },
                    Pending::Git(session, "stash"),
                ),
                Outgoing::Stashes { session } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Stashes { directory },
                    Pending::Stashes(session),
                ),
                Outgoing::CherryPick { session, commit } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::CherryPick { directory, commit },
                    Pending::Git(session, "cherry-pick"),
                ),
                Outgoing::Sync { session, operation } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Sync {
                        directory,
                        operation: operation.to_owned(),
                    },
                    Pending::Remote(session),
                ),
                Outgoing::Log { session, all } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Log {
                        directory,
                        limit: GRAPH_LIMIT,
                        all,
                    },
                    Pending::Log(session),
                ),
                Outgoing::Refresh { session } => self.refresh(model, session),
                Outgoing::Discard { session, paths } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Discard { directory, paths },
                    Pending::Git(session, "discard the changes"),
                ),
                Outgoing::Write {
                    session,
                    path,
                    text,
                } => {
                    let written = path.clone();
                    self.ask_in_folder(
                        model,
                        session,
                        |directory| Request::WriteFile {
                            directory,
                            path,
                            text,
                        },
                        Pending::Write(session, written),
                    );
                }
                Outgoing::Entry { session, change } => {
                    let what = entry_failure(&change);
                    self.ask_in_folder(
                        model,
                        session,
                        |directory| entry_request(change, directory),
                        Pending::Entry(session, what),
                    );
                }
                Outgoing::Ignore { session, paths } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Ignore { directory, paths },
                    Pending::Git(session, "add the files to .gitignore"),
                ),
                Outgoing::Checkout { session, branch } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Checkout { directory, branch },
                    Pending::Git(session, "switch branch"),
                ),
                Outgoing::CreateBranch { session, name } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::CreateBranch { directory, name },
                    Pending::Git(session, "make the branch"),
                ),
                Outgoing::SessionBranches { session } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Branches { directory },
                    Pending::SessionBranches(session),
                ),
                Outgoing::ReadFile {
                    session,
                    path,
                    head,
                } => {
                    let wanted = path.clone();
                    self.ask_in_folder(
                        model,
                        session,
                        |directory| Request::ReadFile {
                            directory,
                            path: wanted,
                            head,
                            revision: None,
                        },
                        Pending::ReadFile(session, path, head),
                    );
                }
                Outgoing::ReadPicture {
                    session,
                    path,
                    head,
                } => {
                    let wanted = path.clone();
                    self.ask_in_folder(
                        model,
                        session,
                        |directory| Request::ReadBytes {
                            directory,
                            path: wanted,
                            head,
                        },
                        Pending::ReadPicture(session, path, head),
                    );
                }
                Outgoing::DrawVector {
                    session,
                    path,
                    head,
                    text,
                } => self.spawn(move |reporter| {
                    let drawn = picture::draw(&path, &text);
                    let _ = reporter.send(Job::Drawn(session, path, head, drawn));
                }),
                Outgoing::DrawDiagram { key, source } => {
                    let palette = self.palette.clone();
                    self.spawn(move |reporter| {
                        let drawn = picture::diagram(&source, &palette);
                        let _ = reporter.send(Job::Figure(key, drawn));
                    });
                }
                Outgoing::DrawFormula { key, source } => {
                    let palette = self.palette.clone();
                    self.spawn(move |reporter| {
                        let drawn = picture::formula(&source, &palette);
                        let _ = reporter.send(Job::Figure(key, drawn));
                    });
                }
                Outgoing::ReadFigure { key, session, path } => self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::ReadBytes {
                        directory,
                        path,
                        head: false,
                    },
                    Pending::Figure(key),
                ),
                Outgoing::FetchFigure { key, address } => self.ask(
                    &None,
                    Request::FetchBytes { url: address },
                    Pending::Figure(key),
                ),
                Outgoing::Accounts { logins, in_use } => self.set_accounts(&logins, &in_use),
                Outgoing::SignIn { agent } => {
                    if let Some(id) = self.agent_id(agent) {
                        self.ask(&None, Request::SignIn { agent: id }, Pending::SignIn(agent));
                    }
                }
                Outgoing::Files {
                    key,
                    host,
                    directory,
                } => self.ask(
                    &host,
                    Request::Files {
                        directory,
                        limit: FILE_LIMIT,
                    },
                    Pending::Files(key),
                ),
                Outgoing::Branches {
                    project,
                    host,
                    directory,
                } => self.ask(
                    &host,
                    Request::Branches { directory },
                    Pending::Branches(project),
                ),
                Outgoing::Skills {
                    agent,
                    project,
                    host,
                    directory,
                } => {
                    if let Some(id) = self.agent_id(agent) {
                        self.ask(
                            &host,
                            Request::Skills {
                                agent: id,
                                directory,
                            },
                            Pending::Skills(agent, project),
                        );
                    }
                }
                Outgoing::Discover { agent, host } => {
                    if let Some(id) = self.agent_id(agent) {
                        self.ask(
                            &host.clone(),
                            Request::Discover { agent: id.clone() },
                            Pending::Discover(host, id),
                        );
                    }
                }
                Outgoing::Identity { agent, naming } => {
                    if let Some(id) = self.agent_id(agent) {
                        self.ask(
                            &None,
                            Request::Identity { agent: id },
                            Pending::Identity(agent, naming),
                        );
                    }
                }
                Outgoing::Open { address } => {
                    if let Err(reason) = pm_acp::open_address(&address) {
                        eprintln!("{reason}");
                    }
                }
                Outgoing::LoadServers => {
                    self.ask(&None, Request::AgentServers, Pending::AgentServers);
                    self.ask(&None, Request::McpServers, Pending::McpServers);
                    self.ask(&None, Request::RegistryAgents, Pending::RegistryAgents);
                    self.ask(
                        &None,
                        Request::SearchTools {
                            query: String::new(),
                        },
                        Pending::Tools(String::new()),
                    );
                }
                Outgoing::SearchTools { query } => self.ask(
                    &None,
                    Request::SearchTools {
                        query: query.clone(),
                    },
                    Pending::Tools(query),
                ),
                Outgoing::SaveAgentServers(servers) => {
                    self.listed(servers.iter().map(|server| server.id.clone()).collect());
                    self.ask(
                        &None,
                        Request::SaveAgentServers { servers },
                        Pending::Nothing,
                    );
                }
                Outgoing::SaveMcpServers(servers) => {
                    self.ask(&None, Request::SaveMcpServers { servers }, Pending::Nothing)
                }
                Outgoing::Owners => self.ask(&None, Request::Owners, Pending::Owners),
                Outgoing::Repositories {
                    login,
                    organization,
                } => self.ask(
                    &None,
                    Request::Repositories {
                        owner: login.clone(),
                        organization,
                    },
                    Pending::Repositories(login),
                ),
                Outgoing::Folders { path, host } => self.ask(
                    &host,
                    Request::Folders { path: path.clone() },
                    Pending::Folders(path),
                ),
                Outgoing::Relist => self.relist(),
                Outgoing::Clone { url, under, host } => self.ask(
                    &host,
                    Request::Clone {
                        url,
                        under: Some(under),
                    },
                    Pending::Added,
                ),
                Outgoing::Copy { text } => copies.push(text),
                Outgoing::SaveMachineSettings(settings) => self.save_machine_settings(settings),
                Outgoing::LatestRelease { channel } => self.ask_release(channel),
                Outgoing::Notify { title, body, how } => crate::notify::tell(how, &title, &body),
                debugging @ (Outgoing::DebugScenarios { .. }
                | Outgoing::DebugProcesses { .. }
                | Outgoing::DebugStart { .. }
                | Outgoing::DebugAttach { .. }
                | Outgoing::DebugDo { .. }) => self.deliver_debug(model, debugging),
                tasking @ (Outgoing::Tasks { .. }
                | Outgoing::RunTask { .. }
                | Outgoing::RunRunnable { .. }
                | Outgoing::Toolchains { .. }
                | Outgoing::SelectToolchain { .. }) => self.deliver_tasks(model, tasking),
                code => self.deliver_code(model, code),
            }
        }
        self.publish_working(model);
        self.watch_shown(model);
        copies
    }

    /// Takes in what the servers and the threads have reported since the last look.
    pub fn poll(&mut self, model: &State<Screen>) {
        self.ask_wanted_grammars(model);
        self.watch_shown(model);
        while let Ok(job) = self.jobs.try_recv() {
            if !matches!(job, Job::Starting(..)) {
                self.pending.set(self.pending.get().saturating_sub(1));
            }
            self.finish(model, job);
        }
        let hosts: Vec<Host> = self.links.keys().cloned().collect();
        for host in hosts {
            let items = match self.links.get(&host).and_then(|link| link.client.as_ref()) {
                Some(client) => client.incoming(),
                None => continue,
            };
            for item in items {
                match item {
                    Incoming::Connected => {
                        self.watched.remove(&host);
                        self.ask(&host, Request::Sessions, Pending::Sessions(host.clone()));
                        self.share_extensions(model, &host);
                        self.ask_extension_assets(&host);
                        self.ask_debugs(&host);
                        model.update(|screen| {
                            *screen = std::mem::take(screen).reopen_documents(&host);
                        });
                    }
                    Incoming::Disconnected(reason) => {
                        eprintln!("the server on {host:?} is out of reach: {reason}");
                        model.update(|screen| {
                            *screen = std::mem::take(screen).with_git_working(false);
                        });
                    }
                    Incoming::Response { id, response } => {
                        let pending = self
                            .links
                            .get_mut(&host)
                            .and_then(|link| link.waiting.remove(&id));
                        if let Some(pending) = pending {
                            self.answered(model, &host, pending, response);
                            self.publish_working(model);
                        }
                    }
                    Incoming::Event {
                        session,
                        seq,
                        event,
                    } => {
                        self.heard(model, &host, session, seq, event);
                    }
                    Incoming::TerminalOutput { terminal, data } => {
                        let key = (host.clone(), terminal);
                        match self.shells.get(&key).copied() {
                            Some(tab) => {
                                model.update(|screen| screen.terminal_output(tab, &data));
                            }
                            None => self.unclaimed.entry(key).or_default().extend(data),
                        }
                    }
                    Incoming::TerminalExited(terminal) => {
                        self.unclaimed.remove(&(host.clone(), terminal));
                        if let Some(tab) = self.shells.remove(&(host.clone(), terminal)) {
                            self.tabs.remove(&tab);
                            model.update(|screen| screen.terminal_exited(tab));
                        }
                    }
                    Incoming::Debug {
                        debug,
                        view,
                        events,
                    } => model.update(|screen| {
                        *screen = std::mem::take(screen).with_debug(debug, view, events);
                    }),
                    Incoming::SessionAdded(info) => self.adopt(model, &host, vec![info]),
                    Incoming::SessionClosed(number) => {
                        if let Some(position) = self.positions.get(&(host.clone(), number)).copied()
                        {
                            model.update(|screen| {
                                *screen = std::mem::take(screen).closed_elsewhere(position)
                            });
                        }
                    }
                    Incoming::ProjectsChanged(paths) => {
                        if host.is_none() {
                            Self::projects_changed(model, &paths);
                        }
                    }
                    Incoming::Diagnostics {
                        directory,
                        path,
                        diagnostics,
                        ..
                    } => model.update(|screen| {
                        *screen = std::mem::take(screen).with_diagnostics(
                            &host,
                            &directory,
                            &path,
                            diagnostics,
                        );
                    }),
                    Incoming::FolderChanged(directory) => {
                        self.folder_changed(model, &host, &directory);
                    }
                    Incoming::ExtensionsChanged(extensions) => {
                        self.ask_extension_assets(&host);
                        model.update(|screen| {
                            *screen =
                                std::mem::take(screen).with_host_extensions(&host, &extensions);
                        });
                        if host.is_none() {
                            model.update(|screen| {
                                *screen = std::mem::take(screen).with_extensions(extensions);
                            });
                        }
                    }
                }
            }
        }
    }

    /// Whether something is yet to be answered, so the screen should keep looking for what it finds.
    pub fn busy(&self) -> bool {
        self.pending.get() > 0
            || self
                .links
                .values()
                .any(|link| link.client.is_none() || !link.waiting.is_empty())
    }

    /// Draws diagrams in the colours of `palette` from now on, and says whether they changed.
    pub fn set_palette(&mut self, palette: picture::Palette) -> bool {
        let changed = palette != self.palette;
        self.palette = palette;
        changed
    }

    /// Runs `work` on a thread of its own, which reports what it found through the sender it is given.
    fn spawn(&self, work: impl FnOnce(Sender<Job>) + Send + 'static) {
        self.pending.set(self.pending.get() + 1);
        let reporter = self.reporter.clone();
        std::thread::spawn(move || work(reporter));
    }

    /// Decodes the pictures among the files at `paths` away from the window, and reports each.
    pub fn preview(&self, paths: Vec<String>) {
        for path in paths {
            self.spawn(move |reporter| {
                let picture = decode_preview(&path);
                let _ = reporter.send(Job::Previewed(path, picture));
            });
        }
    }

    /// Takes in one report of a thread.
    fn finish(&mut self, model: &State<Screen>, job: Job) {
        match job {
            Job::Previewed(path, picture) => {
                if let Some(picture) = picture {
                    model.update(|screen| {
                        *screen = std::mem::take(screen).with_preview(&path, picture)
                    });
                }
            }
            Job::Pictured(session, path, head, read) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_picture(session, &path, head, read);
                });
            }
            Job::Drawn(session, path, head, drawn) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_drawn(session, &path, head, drawn);
                });
            }
            Job::Figure(key, drawn) => {
                model.update(|screen| *screen = std::mem::take(screen).with_figure(key, drawn));
            }
            Job::Server(host, Ok(target)) => {
                if !self.links.contains_key(&host) {
                    return;
                }
                let client = Client::connect(target);
                let first = self.settings_for(&host);
                let Some(link) = self.links.get_mut(&host) else {
                    return;
                };
                if let Some(request) = first {
                    client.request(request);
                }
                for (request, pending) in std::mem::take(&mut link.backlog) {
                    let id = client.request(request);
                    link.waiting.insert(id, pending);
                }
                link.client = Some(client);
            }
            Job::Grammar(language, outcome) => self.grammar_compiled(model, &language, outcome),
            Job::Starting(host, text) => {
                let Some(link) = self.links.get(&host) else {
                    return;
                };
                let starting: Vec<usize> = link
                    .backlog
                    .iter()
                    .filter_map(|(_, pending)| match pending {
                        Pending::Start(session) => Some(*session),
                        _ => None,
                    })
                    .collect();
                for session in starting {
                    let line = format!("{text}\n");
                    model.update(|screen| {
                        *screen = std::mem::take(screen).heard(session, Event::Thought(line));
                    });
                }
            }
            Job::Server(host, Err(reason)) => {
                eprintln!("{reason}");
                let Some(link) = self.links.remove(&host) else {
                    return;
                };
                for (_, pending) in link.backlog {
                    if let Pending::Start(session) = pending {
                        let reason = reason.clone();
                        model.update(|screen| {
                            *screen = std::mem::take(screen).heard(session, Event::Failed(reason));
                        });
                    }
                }
            }
        }
    }

    /// Follows the sessions to their new positions after the window dropped some: `moved` says, for each old position, the new one, or nothing for a session that is gone.
    fn reindex(&mut self, moved: &[Option<usize>]) {
        let before = std::mem::take(&mut self.numbers);
        self.positions.clear();
        self.read_again = std::mem::take(&mut self.read_again)
            .into_iter()
            .filter_map(|old| moved.get(old).copied().flatten())
            .collect();
        for (old, key) in before {
            match moved.get(old).copied().flatten() {
                Some(new) => {
                    self.positions.insert(key.clone(), new);
                    self.numbers.insert(new, key);
                }
                None => {
                    self.early.remove(&key);
                    self.forgotten.insert(key);
                }
            }
        }
    }

    /// Asks every server reached for its sessions again, so that those of a project just added come back.
    pub fn relist(&mut self) {
        let hosts: Vec<Host> = self.links.keys().cloned().collect();
        for host in hosts {
            self.ask(&host, Request::Sessions, Pending::Sessions(host.clone()));
        }
    }

    /// Passes what the agent of the session numbered `number` on `host` did on to the screen.
    fn heard(&mut self, model: &State<Screen>, host: &Host, number: u64, seq: u64, event: Event) {
        let key = (host.clone(), number);
        let live = seq > self.history.get(&key).copied().unwrap_or(0);
        if self.forgotten.contains(&key) {
            return;
        }
        let Some(session) = self.positions.get(&key).copied() else {
            self.early.entry(key).or_default().push((seq, event));
            return;
        };
        let files_moved = matches!(
            event,
            Event::Finished(_) | Event::Failed(_) | Event::Restored { .. }
        );
        let finished = live && matches!(event, Event::Finished(_));
        model.update(|screen| *screen = std::mem::take(screen).heard(session, event));
        if files_moved {
            self.refresh(model, session);
        }
        if finished {
            model.update(|screen| *screen = std::mem::take(screen).agent_finished(session));
        }
    }

    /// Tells the window whether the server is busy with something about a session's folder, when that has changed.
    fn publish_working(&self, model: &State<Screen>) {
        let working = self
            .links
            .values()
            .any(|link| link.waiting.values().any(Pending::is_git));
        if model.read(|screen| screen.git_working()) != working {
            model.update(|screen| *screen = std::mem::take(screen).with_git_working(working));
        }
    }

    /// Does what the answer to a request calls for.
    fn answered(
        &mut self,
        model: &State<Screen>,
        host: &Host,
        pending: Pending,
        response: Response,
    ) {
        let failure = match &response {
            Response::Failed { message } => Some(message.clone()),
            _ => None,
        };
        match pending {
            Pending::Nothing => {
                if let Some(message) = failure {
                    eprintln!("the server could not do what was asked: {message}");
                }
            }
            Pending::Discover(origin, id) => {
                let found = match response {
                    Response::Discovery(found) => found,
                    _ => Discovery::default(),
                };
                if origin.is_none() && !found.settings.is_empty() {
                    let _ = pm_store::save_text(&format!("discovery-{id}"), &found.to_text());
                }
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_discovery_of(origin, &id, found);
                });
            }
            Pending::SignIn(agent) => {
                model.update(|screen| *screen = std::mem::take(screen).signed_in(agent));
                if let Some(message) = failure {
                    eprintln!("sign-in failed: {message}");
                } else if let Some(id) = self.agent_id(agent) {
                    self.ask(
                        &None,
                        Request::Discover { agent: id.clone() },
                        Pending::Discover(None, id),
                    );
                }
            }
            Pending::Sessions(origin) => {
                if let Response::Sessions { sessions } = response {
                    self.adopt(model, &origin, sessions);
                    self.ask(
                        &origin,
                        Request::Terminals,
                        Pending::Terminals(origin.clone()),
                    );
                }
            }
            Pending::OpenTerminal(tab) => match response {
                Response::Terminal { terminal } => self.opened_terminal(model, host, tab, terminal),
                Response::Failed { message } => {
                    self.held.remove(&tab);
                    let report = format!("\r\ncould not start a shell: {message}\r\n");
                    model.update(|screen| screen.terminal_output(tab, report.as_bytes()));
                }
                _ => {}
            },
            Pending::Terminals(origin) => {
                if let Response::Terminals { terminals } = response {
                    self.adopt_terminals(model, &origin, terminals);
                }
            }
            Pending::Skills(agent, project) => {
                if let Response::Skills { skills, names } = response {
                    model.update(|screen| {
                        *screen = std::mem::take(screen).with_skills(
                            agent,
                            project,
                            skills,
                            names.into_iter().collect(),
                        );
                    });
                }
            }
            Pending::AgentServers => {
                if let Response::AgentServers { servers } = response {
                    self.listed(servers.iter().map(|server| server.id.clone()).collect());
                    Self::in_workspace(model, |state| state.with_agent_servers(servers));
                }
            }
            Pending::McpServers => {
                if let Response::McpServers { servers } = response {
                    Self::in_workspace(model, |state| state.with_mcp_servers(servers));
                }
            }
            Pending::RegistryAgents => {
                let answer = match response {
                    Response::RegistryAgents { agents } => Ok(agents),
                    other => Err(Self::trouble_of(other)),
                };
                Self::in_workspace(model, |state| state.with_registry_agents(answer));
            }
            Pending::Tools(query) => {
                let answer = match response {
                    Response::ToolListings { listings } => Ok(listings),
                    other => Err(Self::trouble_of(other)),
                };
                Self::in_workspace(model, |state| state.with_tool_listings(&query, answer));
            }
            Pending::Owners => {
                let listed = match response {
                    Response::Owners { owners } => Ok(owners
                        .into_iter()
                        .map(|owner| Found {
                            name: owner.login.clone(),
                            detail: match owner.organization {
                                true => "Organization".to_owned(),
                                false => "Personal repositories".to_owned(),
                            },
                            value: owner.login,
                            kind: Kind::Owner(owner.organization),
                        })
                        .collect()),
                    other => Err(Self::trouble_of(other)),
                };
                Self::in_workspace(model, |state| state.with_owners(listed));
            }
            Pending::Repositories(login) => {
                let listed = match response {
                    Response::Repositories { repositories } => Ok(repositories
                        .into_iter()
                        .map(|repository| Found {
                            name: repository.name,
                            detail: match repository.private {
                                true => format!("Private · {}", repository.description),
                                false => repository.description,
                            },
                            value: repository.url,
                            kind: Kind::Repository,
                        })
                        .collect()),
                    other => Err(Self::trouble_of(other)),
                };
                Self::in_workspace(model, |state| state.with_repositories(&login, listed));
            }
            Pending::Folders(requested) => {
                let listed = match response {
                    Response::Folders { path, names } => Ok((path, names)),
                    other => Err(Self::trouble_of(other)),
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_folders(&requested, listed);
                });
            }
            Pending::Added => match response {
                Response::Directory { path } => {
                    Self::in_workspace(model, |state| state.adder_landed(&path));
                    if let Some(paths) = model.get().project_paths() {
                        self.save_projects(paths);
                    }
                    self.relist();
                }
                Response::Failed { message } => {
                    Self::in_workspace(model, |state| state.adder_failed(&message));
                }
                _ => {}
            },
            Pending::Files(key) => {
                if let Response::Files {
                    paths,
                    folders,
                    ignored,
                } = response
                {
                    model.update(|screen| {
                        *screen = std::mem::take(screen).with_files(&key, paths, folders, ignored);
                    });
                }
            }
            Pending::Branches(project) => {
                if let Response::Branches { names } = response {
                    model.update(|screen| {
                        *screen = std::mem::take(screen).with_branches(project, names)
                    });
                }
            }
            Pending::Identity(agent, naming) => {
                if let Response::Identity { default, selected } = response {
                    model.update(|screen| {
                        *screen = screen
                            .clone()
                            .with_identity(agent, default, selected, naming);
                    });
                }
            }
            Pending::Start(session) => match response {
                Response::Started {
                    session: number,
                    directory,
                    branch,
                    head,
                } => {
                    self.numbers.insert(session, (host.clone(), number));
                    self.positions.insert((host.clone(), number), session);
                    if let Some(client) = self.links.get(host).and_then(|link| link.client.as_ref())
                    {
                        client.watch(number, 0);
                    }
                    model.update(|screen| {
                        *screen = std::mem::take(screen).worktree_ready(
                            session,
                            &directory,
                            branch.as_deref(),
                            head.as_deref(),
                        );
                    });
                    for (seq, event) in self
                        .early
                        .remove(&(host.clone(), number))
                        .unwrap_or_default()
                    {
                        self.heard(model, host, number, seq, event);
                    }
                }
                Response::Failed { message } => model.update(|screen| {
                    *screen = std::mem::take(screen).heard(session, Event::Failed(message));
                }),
                _ => {}
            },
            Pending::Inspect(session) => {
                if let Response::Inspected {
                    changes,
                    commits,
                    branch,
                    head,
                    tracking,
                    operation,
                } = response
                {
                    model.update(|screen| {
                        *screen = std::mem::take(screen)
                            .refreshed(session, &changes, commits)
                            .checked_out(session, branch.as_deref(), head.as_deref())
                            .tracked(session, branch.as_deref(), tracking, operation);
                    });
                }
                if self.read_again.remove(&session) {
                    self.refresh(model, session);
                }
            }
            Pending::Stage(session) => {
                if let Some(message) = failure {
                    let reason = format!("Could not stage the files: {message}");
                    model.update(|screen| *screen = std::mem::take(screen).git_failed(&reason));
                }
                self.refresh(model, session);
            }
            Pending::Git(session, what) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).remote_done(session, None);
                });
                if let Some(message) = failure {
                    let reason = format!("Could not {what}: {message}");
                    model.update(|screen| *screen = std::mem::take(screen).git_failed(&reason));
                }
                self.refresh(model, session);
            }
            Pending::Write(session, path) => {
                let failed = failure.is_some();
                if let Some(message) = failure {
                    let reason = format!("Could not save {path}: {message}");
                    model.update(|screen| *screen = std::mem::take(screen).git_failed(&reason));
                }
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_file_written(session, &path, failed);
                });
                self.refresh(model, session);
            }
            Pending::Entry(session, what) => {
                if let Response::Edited { files } = response {
                    model.update(|screen| {
                        *screen = std::mem::take(screen).with_edited_files(session, files);
                    });
                } else if let Some(message) = failure {
                    let reason = format!("Could not {what}: {message}");
                    model.update(|screen| *screen = std::mem::take(screen).git_failed(&reason));
                }
                model.update(|screen| *screen = std::mem::take(screen).with_tree_stale());
                self.refresh(model, session);
            }
            Pending::SessionBranches(session) => {
                if let Response::Branches { names } = response {
                    model.update(|screen| {
                        *screen = std::mem::take(screen).with_session_branches(session, names);
                    });
                }
            }
            Pending::ReadFile(session, path, head) => {
                let read = match response {
                    Response::File {
                        text,
                        truncated,
                        language,
                    } => Ok((text, truncated, language)),
                    Response::Failed { message } => Err(message),
                    _ => Err("the server answered with something else".to_owned()),
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_file(session, &path, head, read);
                });
            }
            Pending::ReadPicture(session, path, head) => match response {
                Response::Bytes { bytes, truncated } => self.spawn(move |reporter| {
                    let read = picture::decode(&path, &bytes, truncated);
                    let _ = reporter.send(Job::Pictured(session, path, head, read));
                }),
                other => {
                    let reason = match other {
                        Response::Failed { message } => message,
                        _ => "the server answered with something else".to_owned(),
                    };
                    model.update(|screen| {
                        *screen =
                            std::mem::take(screen).with_picture(session, &path, head, Err(reason));
                    });
                }
            },
            Pending::Figure(key) => {
                let read = match response {
                    Response::Bytes { bytes, truncated } => Ok((bytes, truncated)),
                    Response::Failed { message } => Err(message),
                    _ => Err("the server answered with something else".to_owned()),
                };
                match read {
                    Ok((bytes, truncated)) => self.spawn(move |reporter| {
                        let drawn = picture::figure(&bytes, truncated);
                        let _ = reporter.send(Job::Figure(key, drawn));
                    }),
                    Err(reason) => model.update(|screen| {
                        *screen = std::mem::take(screen).with_figure(key, Err(reason));
                    }),
                }
            }
            Pending::Commit(session, push) => {
                let failed = failure.is_some();
                if let Some(message) = failure {
                    let reason = format!("The commit failed: {message}");
                    model.update(|screen| *screen = std::mem::take(screen).git_failed(&reason));
                }
                if failed || !push {
                    model.update(|screen| {
                        *screen = std::mem::take(screen).remote_done(session, None);
                    });
                }
                self.refresh(model, session);
                if push && !failed {
                    self.ask_in_folder(
                        model,
                        session,
                        |directory| Request::Sync {
                            directory,
                            operation: "push".to_owned(),
                        },
                        Pending::Remote(session),
                    );
                }
            }
            Pending::Remote(session) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).remote_done(session, failure);
                });
                self.refresh(model, session);
            }
            Pending::Stashes(session) => {
                if let Response::Stashes { stashes } = response {
                    model.update(|screen| {
                        *screen = std::mem::take(screen).with_stashes(session, stashes);
                    });
                }
            }
            Pending::Log(session) => {
                if let Response::Log { commits } = response {
                    model.update(|screen| {
                        *screen = std::mem::take(screen).with_history(session, commits);
                    });
                }
            }
            Pending::Fork(session) => match response {
                Response::Sessions { sessions } => {
                    let branched = sessions.first().map(|info| (host.clone(), info.session));
                    self.adopt(model, host, sessions);
                    if let Some(position) = branched.and_then(|key| self.positions.get(&key)) {
                        let open = FocusId(SESSION_BASE + *position as u64);
                        model.update(|screen| *screen = std::mem::take(screen).after(open));
                    }
                }
                _ => {
                    let reason = format!(
                        "Could not branch from this reply: {}",
                        failure.unwrap_or_else(|| "the server answered something else".to_owned())
                    );
                    model.update(|screen| *screen = std::mem::take(screen).noted(session, &reason));
                }
            },
            Pending::Grammar(_)
            | Pending::Opened(..)
            | Pending::Completions(..)
            | Pending::Hovered(..)
            | Pending::Definitions(..)
            | Pending::Base(..)
            | Pending::Symbols(..)
            | Pending::Problems(_)
            | Pending::Blame(..)
            | Pending::Highlights(..)
            | Pending::Signature(..)
            | Pending::ProjectSearch(..)
            | Pending::SemanticTokens(..)
            | Pending::InlayHints(..)
            | Pending::InlineValues(..)
            | Pending::CodeLenses(..)
            | Pending::LensPressed(_)
            | Pending::Renamed(_)
            | Pending::CodeActions(..)
            | Pending::TookAction(_)
            | Pending::Permalink(_)
            | Pending::FileHistory(..)
            | Pending::Revision(..)
            | Pending::LocalHistory(..)
            | Pending::Version(..)
            | Pending::Conflict(..)
            | Pending::Revisions(..)
            | Pending::Formatted(..)
            | Pending::FoldingRanges(..)
            | Pending::LinkedRanges(..)
            | Pending::Extensions
            | Pending::ExtensionPreview(_)
            | Pending::ExtensionAssets(_)
            | Pending::ExtensionCommand(_) => self.answered_code(model, host, pending, response),
            Pending::DebugScenarios(_)
            | Pending::DebugProcesses(_)
            | Pending::DebugStarted(_)
            | Pending::Debugs(_)
            | Pending::DebugWatched(_)
            | Pending::DebugRewatched(_) => self.answered_debug(model, pending, response),
            Pending::Tasks(_) | Pending::Toolchains(_) => {
                self.answered_tasks(model, pending, response)
            }
            Pending::MachineSettings | Pending::LatestRelease => {
                self.answered_machine(model, pending, response)
            }
            Pending::Back(session, failing) => {
                if let Some(message) = failure {
                    let reason = format!("{failing}: {message}");
                    model.update(|screen| *screen = std::mem::take(screen).noted(session, &reason));
                }
            }
        }
    }

    /// Sends what is done to the shell of `tab` to the server that runs it, or keeps it until the server says which shell that is.
    fn ask_terminal(&mut self, tab: u64, item: Held) {
        if let Some(queue) = self.held.get_mut(&tab) {
            queue.push(item);
            return;
        }
        if let Some((host, terminal)) = self.tabs.get(&tab).cloned() {
            self.send_to_terminal(&host, tab, terminal, item);
        }
    }

    /// Asks the server on `host` to do `item` to its shell numbered `terminal`, which is the shell of `tab`.
    fn send_to_terminal(&mut self, host: &Host, tab: u64, terminal: u64, item: Held) {
        let request = match item {
            Held::Input(data) => Request::TerminalInput { terminal, data },
            Held::Resize(columns, rows) => Request::ResizeTerminal {
                terminal,
                columns,
                rows,
            },
            Held::Close => {
                self.tabs.remove(&tab);
                self.shells.remove(&(host.clone(), terminal));
                Request::CloseTerminal { terminal }
            }
        };
        self.ask(host, request, Pending::Nothing);
    }

    /// Takes in the server's number for the shell of `tab`: what was done to it meanwhile is sent, and what it printed meanwhile is shown.
    fn opened_terminal(&mut self, model: &State<Screen>, host: &Host, tab: u64, terminal: u64) {
        self.tabs.insert(tab, (host.clone(), terminal));
        self.shells.insert((host.clone(), terminal), tab);
        if let Some(client) = self.links.get(host).and_then(|link| link.client.as_ref()) {
            client.watch_terminal(terminal, 0);
        }
        if let Some(data) = self.unclaimed.remove(&(host.clone(), terminal)) {
            model.update(|screen| screen.terminal_output(tab, &data));
        }
        for item in self.held.remove(&tab).unwrap_or_default() {
            self.send_to_terminal(host, tab, terminal, item);
        }
    }

    /// Adds a tab for each shell of the server on `origin` that runs in the folder of a session the window shows and has no tab yet, and starts a fresh one when the dock was left open on a session whose shells are gone.
    fn adopt_terminals(
        &mut self,
        model: &State<Screen>,
        origin: &Host,
        terminals: Vec<pm_protocol::TerminalInfo>,
    ) {
        for info in terminals {
            let key = (origin.clone(), info.terminal);
            if self.shells.contains_key(&key) {
                continue;
            }
            let Some(session) = model.get().session_in(origin, &info.directory) else {
                continue;
            };
            let mut adopted = None;
            model.update(|screen| {
                adopted = screen.adopt_terminal(session, info.columns, info.rows);
            });
            let Some(tab) = adopted else {
                continue;
            };
            self.tabs.insert(tab, key.clone());
            self.shells.insert(key.clone(), tab);
            if let Some(client) = self.links.get(origin).and_then(|link| link.client.as_ref()) {
                client.watch_terminal(info.terminal, 0);
            }
        }
        model.update(|screen| screen.revive_terminal(origin));
    }

    /// Adds the sessions of the server of `origin` that the window does not show yet, and starts watching them.
    fn adopt(
        &mut self,
        model: &State<Screen>,
        origin: &Host,
        sessions: Vec<pm_protocol::SessionInfo>,
    ) {
        let fresh: Vec<_> = sessions
            .into_iter()
            .filter(|info| !self.positions.contains_key(&(origin.clone(), info.session)))
            .collect();
        for info in &fresh {
            self.history
                .insert((origin.clone(), info.session), info.last_seq);
        }
        let mut added = Vec::new();
        model.update(|screen| {
            let (next, mapping) = std::mem::take(screen).restored(origin, &fresh);
            *screen = next;
            added = mapping;
        });
        for (position, number) in added {
            self.forgotten.remove(&(origin.clone(), number));
            self.numbers.insert(position, (origin.clone(), number));
            self.positions.insert((origin.clone(), number), position);
            if let Some(client) = self.links.get(origin).and_then(|link| link.client.as_ref()) {
                client.watch(number, 0);
            }
        }
        self.finish_resuming(model, origin);
    }

    /// Opens the session the window was left on, and scrolls the sidebar as it was, once the server of `origin` that holds the session has listed its sessions.
    fn finish_resuming(&mut self, model: &State<Screen>, origin: &Host) {
        if self
            .resume
            .as_ref()
            .is_none_or(|view| view.home() != *origin)
        {
            return;
        }
        let Some(view) = self.resume.take() else {
            return;
        };
        let position = view
            .session()
            .and_then(|key| self.positions.get(&key).copied());
        model.update(|screen| {
            if let Screen::Workspace(state) = screen {
                *state = view.reopen(std::mem::replace(state, WorkspaceState::new(&[])), position);
            }
        });
    }

    /// Why `response` is not the answer that was waited for.
    fn trouble_of(response: Response) -> String {
        match response {
            Response::Failed { message } => message,
            _ => "the server answered with something else".to_owned(),
        }
    }

    /// Replaces the workspace with what `change` makes of it, while the workspace is showing.
    fn in_workspace(model: &State<Screen>, change: impl FnOnce(WorkspaceState) -> WorkspaceState) {
        model.update(|screen| {
            if let Screen::Workspace(state) = screen {
                *state = change(std::mem::replace(state, WorkspaceState::new(&[])));
            }
        });
    }

    /// Adds the projects another client added on this machine.
    fn projects_changed(model: &State<Screen>, paths: &[String]) {
        model.update(|screen| {
            let mut next = std::mem::take(screen);
            for path in paths {
                next = next.with_project(path);
            }
            *screen = next;
        });
    }

    /// Where the session at `session` works.
    fn session_of(model: &State<Screen>, session: usize) -> Option<Placement> {
        let screen = model.get();
        let Screen::Workspace(state) = &screen else {
            return None;
        };
        let held = state.sessions.get(session)?;
        let project = state.projects.get(held.project)?;
        Some(Placement {
            host: project.host(),
            directory: held.directory.clone(),
            agent: held.agent,
            base: held.base.clone(),
            repository: project.directory().to_string_lossy().into_owned(),
            title: held.title.clone(),
        })
    }

    /// Asks the machine the session at `session` is on for what `make` builds from its folder.
    fn ask_in_folder(
        &mut self,
        model: &State<Screen>,
        session: usize,
        make: impl FnOnce(String) -> Request,
        pending: Pending,
    ) {
        if let Some(placement) = Self::session_of(model, session) {
            self.ask(&placement.host, make(placement.directory), pending);
        }
    }

    /// Looks at the folder of the session at `session` again, for its changes and commits.
    fn refresh(&mut self, model: &State<Screen>, session: usize) {
        let Some(placement) = Self::session_of(model, session) else {
            return;
        };
        model.update(|screen| *screen = std::mem::take(screen).bases_wanted(session));
        let inspecting = self.links.get(&placement.host).is_some_and(|link| {
            link.waiting
                .values()
                .any(|pending| matches!(pending, Pending::Inspect(held) if *held == session))
        });
        if inspecting {
            self.read_again.insert(session);
            return;
        }
        let log = model.read(|screen| screen.graph_wanted());
        self.ask(
            &placement.host,
            Request::Inspect {
                directory: placement.directory.clone(),
                base: placement.base,
            },
            Pending::Inspect(session),
        );
        if let Some(all) = log {
            self.ask(
                &placement.host,
                Request::Log {
                    directory: placement.directory,
                    limit: GRAPH_LIMIT,
                    all,
                },
                Pending::Log(session),
            );
        }
    }
}

/// The longest side of a decoded preview, in pixels.
const PREVIEW_SIDE: u32 = 512;

/// The picture in the file at `path`, shrunk to a preview, or nothing when the file is not a picture.
fn decode_preview(path: &str) -> Option<Picture> {
    use std::hash::{Hash, Hasher};
    let modified = std::fs::metadata(path)
        .and_then(|data| data.modified())
        .ok();
    let decoded = image::open(path)
        .ok()?
        .thumbnail(PREVIEW_SIDE, PREVIEW_SIDE);
    let rgba = decoded.to_rgba8();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (path, modified).hash(&mut hasher);
    Some(Picture {
        key: hasher.finish(),
        width: rgba.width(),
        height: rgba.height(),
        pixels: std::sync::Arc::new(rgba.into_raw()),
    })
}

/// Shows `directory` in the file manager of this machine.
fn show_in_file_manager(directory: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener)
        .arg(directory)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
