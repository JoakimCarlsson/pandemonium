//! The work behind the screen: asking the servers to run the agents and the git operations, and taking in what they answer.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};

use pm_acp::{AGENTS, Discovery, Event};
use pm_client::{Client, Incoming};
use pm_components::adder::{Found, Kind};
use pm_components::state::SESSION_BASE;
use pm_input::FocusId;
use pm_pages::{Outgoing, Screen, WorkspaceState};
use pm_protocol::{Attachment, Login, Request, Response, Target};
use pm_renderer::Picture;
use pm_state::State;

use crate::view::View;

/// The most files listed for the list behind an `@`.
const FILE_LIMIT: usize = 20_000;

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
}

/// How long the window waits for the server on this machine to say what it keeps.
const BOOTSTRAP_WAIT: std::time::Duration = std::time::Duration::from_secs(4);

/// A machine the servers run on: nothing for this one, or the destination ssh is given for another.
type Host = Option<String>;

/// What a thread that did some of the work reports back.
enum Job {
    /// The picture at this path was decoded, when it is one.
    Previewed(String, Option<Picture>),
    /// The server on this machine was found or reached, or could not be.
    Server(Host, Result<Target, String>),
}

/// What is to be done with the answer to a request.
#[derive(Clone, Debug)]
enum Pending {
    /// Nothing: the answer only matters if it says the request failed.
    Nothing,
    /// The agent at this position told what it lets the reader change.
    Discover(Host, usize),
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
    Commit(usize),
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
    /// The machine and the server's number of each session, by the session's position.
    numbers: HashMap<usize, (Host, u64)>,
    /// The session's position, by the machine and the server's number.
    positions: HashMap<(Host, u64), usize>,
    /// The sessions the window let go of with their project, whose events are not wanted until the project is added again.
    forgotten: HashSet<(Host, u64)>,
    /// What agents did before their session was known by position.
    early: HashMap<(Host, u64), Vec<Event>>,
    /// What the threads report.
    jobs: Receiver<Job>,
    /// Handed to each thread.
    reporter: Sender<Job>,
    /// Whether the agents on this machine have been asked what they let the reader change.
    discovered: bool,
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
}

impl Work {
    /// No servers reached yet: they are looked for when something is first asked of them.
    pub fn new() -> Self {
        let (reporter, jobs) = mpsc::channel();
        Self {
            links: HashMap::new(),
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
            resume: None,
            pending: Cell::new(0),
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
        let mut projects = None;
        let mut choices = None;
        let mut accounts = None;
        let started = std::time::Instant::now();
        while started.elapsed() < BOOTSTRAP_WAIT
            && (projects.is_none() || choices.is_none() || accounts.is_none())
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
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let (projects, choices, (logins, in_use)) = (projects?, choices?, accounts?);
        self.links.insert(
            None,
            Link {
                client: Some(client),
                ..Link::default()
            },
        );
        self.ask(&None, Request::Sessions, Pending::Sessions(None));
        Some(Bootstrap {
            projects,
            choices,
            logins: logins
                .into_iter()
                .map(|login| (login.agent, login.id, login.name))
                .collect(),
            in_use,
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
            self.spawn(move |reporter| {
                let target = match &host {
                    None => pm_client::local(),
                    Some(destination) => pm_ssh::remote(destination),
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
        for (index, agent) in AGENTS.iter().enumerate() {
            self.ask(
                &None,
                Request::Discover {
                    agent: agent.id.to_owned(),
                },
                Pending::Discover(None, index),
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

    /// The agent at `position`, by the id the servers know it by.
    fn agent_id(position: usize) -> Option<String> {
        AGENTS.get(position).map(|agent| agent.id.to_owned())
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
                    tab,
                    columns,
                    rows,
                } => {
                    let Some(placement) = Self::session_of(model, session) else {
                        continue;
                    };
                    let directory = if placement.directory.is_empty() {
                        placement.repository
                    } else {
                        placement.directory
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
                Outgoing::Close { session } => self.ask_session(
                    session,
                    |number| Request::CloseSession { session: number },
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
                    let Some(agent) = Self::agent_id(placement.agent) else {
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
                Outgoing::Commit { session, message } => {
                    if let Some(placement) = Self::session_of(model, session) {
                        self.ask(
                            &placement.host,
                            Request::Commit {
                                directory: placement.directory,
                                message,
                            },
                            Pending::Commit(session),
                        );
                    }
                }
                Outgoing::Accounts { logins, in_use } => self.set_accounts(&logins, &in_use),
                Outgoing::SignIn { agent } => {
                    if let Some(id) = Self::agent_id(agent) {
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
                    if let Some(id) = Self::agent_id(agent) {
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
                    if let Some(id) = Self::agent_id(agent) {
                        self.ask(
                            &host.clone(),
                            Request::Discover { agent: id },
                            Pending::Discover(host, agent),
                        );
                    }
                }
                Outgoing::Identity { agent, naming } => {
                    if let Some(id) = Self::agent_id(agent) {
                        self.ask(
                            &None,
                            Request::Identity { agent: id },
                            Pending::Identity(agent, naming),
                        );
                    }
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
                Outgoing::Folders { path } => self.ask(
                    &None,
                    Request::Folders { path: path.clone() },
                    Pending::Folders(path),
                ),
                Outgoing::Relist => self.relist(),
                Outgoing::Clone { url, under } => self.ask(
                    &None,
                    Request::Clone {
                        url,
                        under: Some(under),
                    },
                    Pending::Added,
                ),
                Outgoing::Copy { text } => copies.push(text),
            }
        }
        copies
    }

    /// Takes in what the servers and the threads have reported since the last look.
    pub fn poll(&mut self, model: &State<Screen>) {
        while let Ok(job) = self.jobs.try_recv() {
            self.pending.set(self.pending.get().saturating_sub(1));
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
                        self.ask(&host, Request::Sessions, Pending::Sessions(host.clone()));
                    }
                    Incoming::Disconnected(reason) => {
                        eprintln!("the server on {host:?} is out of reach: {reason}");
                    }
                    Incoming::Response { id, response } => {
                        let pending = self
                            .links
                            .get_mut(&host)
                            .and_then(|link| link.waiting.remove(&id));
                        if let Some(pending) = pending {
                            self.answered(model, &host, pending, response);
                        }
                    }
                    Incoming::Event { session, event, .. } => {
                        self.heard(model, &host, session, event);
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
                    Incoming::SessionAdded(info) => self.adopt(model, &host, vec![info]),
                    Incoming::SessionClosed(number) => {
                        if let Some(position) = self.positions.get(&(host.clone(), number)).copied()
                        {
                            model.update(|screen| {
                                *screen = screen.clone().closed_elsewhere(position)
                            });
                        }
                    }
                    Incoming::ProjectsChanged(paths) => {
                        if host.is_none() {
                            Self::projects_changed(model, &paths);
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
                    model.update(|screen| *screen = screen.clone().with_preview(&path, picture));
                }
            }
            Job::Server(host, Ok(target)) => {
                let Some(link) = self.links.get_mut(&host) else {
                    return;
                };
                let client = Client::connect(target);
                for (request, pending) in std::mem::take(&mut link.backlog) {
                    let id = client.request(request);
                    link.waiting.insert(id, pending);
                }
                link.client = Some(client);
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
                            *screen = screen.clone().heard(session, Event::Failed(reason));
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
    fn heard(&mut self, model: &State<Screen>, host: &Host, number: u64, event: Event) {
        let key = (host.clone(), number);
        if self.forgotten.contains(&key) {
            return;
        }
        let Some(session) = self.positions.get(&key).copied() else {
            self.early.entry(key).or_default().push(event);
            return;
        };
        let files_moved = matches!(
            event,
            Event::Finished(_) | Event::Failed(_) | Event::Restored { .. }
        );
        model.update(|screen| *screen = screen.clone().heard(session, event));
        if files_moved {
            self.refresh(model, session);
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
            Pending::Discover(origin, agent) => {
                let found = match response {
                    Response::Discovery(found) => found,
                    _ => Discovery::default(),
                };
                if origin.is_none()
                    && !found.settings.is_empty()
                    && let Some(held) = AGENTS.get(agent)
                {
                    let _ =
                        pm_store::save_text(&format!("discovery-{}", held.id), &found.to_text());
                }
                model.update(|screen| {
                    *screen = screen.clone().with_discovery_for(origin, agent, found);
                });
            }
            Pending::SignIn(agent) => {
                model.update(|screen| *screen = screen.clone().signed_in(agent));
                if let Some(message) = failure {
                    eprintln!("sign-in failed: {message}");
                } else if let Some(id) = Self::agent_id(agent) {
                    self.ask(
                        &None,
                        Request::Discover { agent: id },
                        Pending::Discover(None, agent),
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
                        *screen = screen.clone().with_skills(
                            agent,
                            project,
                            skills,
                            names.into_iter().collect(),
                        );
                    });
                }
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
                Self::in_workspace(model, |state| state.with_folders(&requested, listed));
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
                if let Response::Files { paths } = response {
                    model.update(|screen| *screen = screen.clone().with_files(&key, paths));
                }
            }
            Pending::Branches(project) => {
                if let Response::Branches { names } = response {
                    model.update(|screen| *screen = screen.clone().with_branches(project, names));
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
                        *screen = screen.clone().worktree_ready(
                            session,
                            &directory,
                            branch.as_deref(),
                            head.as_deref(),
                        );
                    });
                    for event in self
                        .early
                        .remove(&(host.clone(), number))
                        .unwrap_or_default()
                    {
                        self.heard(model, host, number, event);
                    }
                }
                Response::Failed { message } => model.update(|screen| {
                    *screen = screen.clone().heard(session, Event::Failed(message));
                }),
                _ => {}
            },
            Pending::Inspect(session) => {
                if let Response::Inspected {
                    changes,
                    commits,
                    branch,
                    head,
                } = response
                {
                    model.update(|screen| {
                        *screen = screen
                            .clone()
                            .refreshed(session, &changes, commits)
                            .checked_out(session, branch.as_deref(), head.as_deref());
                    });
                }
            }
            Pending::Stage(session) => {
                if let Some(message) = failure {
                    let reason = format!("Could not stage the files: {message}");
                    model.update(|screen| *screen = screen.clone().noted(session, &reason));
                }
                self.refresh(model, session);
            }
            Pending::Commit(session) => {
                if let Some(message) = failure {
                    let reason = format!("The commit failed: {message}");
                    model.update(|screen| *screen = screen.clone().noted(session, &reason));
                }
                self.refresh(model, session);
            }
            Pending::Fork(session) => match response {
                Response::Sessions { sessions } => {
                    let branched = sessions.first().map(|info| (host.clone(), info.session));
                    self.adopt(model, host, sessions);
                    if let Some(position) = branched.and_then(|key| self.positions.get(&key)) {
                        let open = FocusId(SESSION_BASE + *position as u64);
                        model.update(|screen| *screen = screen.clone().after(open));
                    }
                }
                _ => {
                    let reason = format!(
                        "Could not branch from this reply: {}",
                        failure.unwrap_or_else(|| "the server answered something else".to_owned())
                    );
                    model.update(|screen| *screen = screen.clone().noted(session, &reason));
                }
            },
            Pending::Back(session, failing) => {
                if let Some(message) = failure {
                    let reason = format!("{failing}: {message}");
                    model.update(|screen| *screen = screen.clone().noted(session, &reason));
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

    /// Adds a tab for each shell of the server on `origin` that runs in the folder of a session the window shows and has no tab yet.
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
        let mut added = Vec::new();
        model.update(|screen| {
            let (next, mapping) = screen.clone().restored(origin, &fresh);
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
            let mut next = screen.clone();
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

    /// Looks at the folder of the session at `session` again, for its changes and commits.
    fn refresh(&mut self, model: &State<Screen>, session: usize) {
        let Some(placement) = Self::session_of(model, session) else {
            return;
        };
        let inspecting = self.links.get(&placement.host).is_some_and(|link| {
            link.waiting
                .values()
                .any(|pending| matches!(pending, Pending::Inspect(held) if *held == session))
        });
        if inspecting {
            return;
        }
        self.ask(
            &placement.host,
            Request::Inspect {
                directory: placement.directory,
                base: placement.base,
            },
            Pending::Inspect(session),
        );
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
