//! Language servers: starting them, and keeping one per worktree.
//!
//! A server belongs to a worktree and a language, not to a file: every Rust
//! file of one checkout is served by the one rust-analyzer that was started
//! for it. [`Servers`] is that seam — the only place a server process is
//! started, found or ended.
//!
//! A language names every server that answers for it, and all of them that
//! are installed run together: a Python checkout is told about by its type
//! checker and its linter at once, and a file collects what both of them say
//! rather than only the first. A reader who wants other servers than the
//! ones a language names says so through [`Servers::set_overrides`], which
//! replaces that language's list outright, or through [`Servers::set_added`],
//! which runs more servers after the ones the language names.

#![allow(
    clippy::mutable_key_type,
    reason = "Host equality and hashing use immutable SSH aliases only."
)]

mod answer;
mod capabilities;
mod client;
mod database;
mod encoding;
mod log;
mod outbox;
mod progress;
mod rpc;
mod state;
mod sync;
mod uri;
mod watch;

use pm_host::Location as HostLocation;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use answer::{
    Answer, Calls, CodeAction, Completion, CompletionKind, FileEdit, Handle, Lens, Location,
    NamedLocation, Request, Semantic, Signature, Symbol, Trigger, WorkspaceChange,
};
pub use client::{Asked, Client};
pub use log::{is_tracing, set_trace};
pub use progress::Progress;
pub use state::{ServerState, ServerStatus};
pub use watch::Watched;

/// How many consecutive exits are allowed before a server is abandoned.
const RESTART_LIMIT: u32 = 4;

/// A server startup performed away from the window thread.
type Starting = std::sync::mpsc::Receiver<Result<Option<(PathBuf, Client)>, String>>;

/// One server slot and its bounded restart schedule.
#[derive(Default)]
struct Running {
    /// A server startup completed away from the window thread.
    starting: Option<Starting>,
    /// The client currently occupying the slot.
    client: Option<Arc<Client>>,
    /// How often a client in this slot has died.
    failures: u32,
    /// The earliest time another start may be tried.
    retry_at: Option<Instant>,
    /// Declaration used to start this slot.
    server: Option<Server>,
    /// Executable backing the running client.
    program: Option<PathBuf>,
    /// A terminal state retained after the client disappears.
    stopped: Option<ServerState>,
    /// Whether this slot's failure still needs to be reported.
    unreported: bool,
    /// The programs the server runs in turn that were missing when it
    /// started, which restart it once they are installed.
    lacking: Vec<&'static str>,
}

use crate::language::{Language, Server};
use crate::program::{Need, installed_with_recipe, missing_for};

/// The language servers a window is running.
#[derive(Default)]
pub struct Servers {
    /// One server per worktree and language, by the command that started it.
    running: HashMap<(HostLocation, &'static str), Running>,
    /// The servers to run for a language, where that was overridden by name.
    overrides: HashMap<&'static str, Vec<Server>>,
    /// The servers to run for a language after the ones it names.
    added: HashMap<&'static str, Vec<Server>>,
    /// How a server wakes the window once it has something to say.
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Missing commands observed while opening documents.
    missing: HashSet<&'static str>,
    /// Installable programs that started servers run in turn but lack.
    missing_tools: HashSet<&'static Need>,
    /// Worktrees with an open document of each language.
    opened: HashMap<&'static str, HashSet<HostLocation>>,
    /// The directory every server's log is written in, when logs are kept.
    logs: Option<PathBuf>,
}

impl Servers {
    /// Wakes the window through `notify` when a server says something.
    pub fn set_notify(&mut self, notify: Arc<dyn Fn() + Send + Sync>) {
        self.notify = Some(notify);
    }

    /// Writes every server's log into `directory`.
    pub fn set_logs(&mut self, directory: PathBuf) {
        self.logs = Some(directory);
    }

    /// Runs `overrides` in place of what the languages named by name.
    ///
    /// What a reader configured is not merged with what the editor ships:
    /// naming the servers for a language is how a reader turns one off, and
    /// a list that was added to could not do that.
    pub fn set_overrides(&mut self, overrides: HashMap<&'static str, Vec<Server>>) {
        self.overrides = overrides;
        self.reconfigure();
    }

    /// Runs `added` for the languages they name, after the servers those
    /// languages name.
    ///
    /// An override still replaces the list outright: servers added for a
    /// language that was also overridden are not run.
    pub fn set_added(&mut self, added: HashMap<&'static str, Vec<Server>>) {
        self.added = added;
        self.reconfigure();
    }

    /// Hands every running server the settings it is now configured with,
    /// so that a change to them takes without a restart.
    fn reconfigure(&self) {
        for ((_, command), running) in &self.running {
            if let (Some(client), Some(server)) = (&running.client, self.wanted_server(command)) {
                client.configure(server.options);
            }
        }
    }

    /// Every server for `language` over `root`, started if not running.
    ///
    /// A server that is not installed is not asked for, and a server that
    /// will not start is remembered as one that will not start: a missing
    /// rust-analyzer is tried once per worktree, not once per file opened.
    pub fn open(&mut self, root: &HostLocation, language: Language) -> Vec<Arc<Client>> {
        self.opened
            .entry(language.name())
            .or_default()
            .insert(root.clone());
        let Some(notify) = self.notify.clone() else {
            return Vec::new();
        };
        let mut clients = Vec::new();
        for server in self.wanted(language) {
            let running = self
                .running
                .entry((root.clone(), server.command))
                .or_default();
            if let Some(starting) = &running.starting {
                match starting.try_recv() {
                    Ok(result) => {
                        running.starting = None;
                        match result {
                            Ok(Some((program, client))) => {
                                running.program = Some(program);
                                running.client = Some(Arc::new(client));
                                running.stopped = None;
                            }
                            Ok(None) => {
                                running.stopped = Some(ServerState::Missing);
                                if root.host.is_local() {
                                    self.missing.insert(server.command);
                                }
                            }
                            Err(reason) => {
                                running.stopped = Some(ServerState::Failed { reason });
                                running.unreported = true;
                            }
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        running.starting = None;
                        running.stopped = Some(ServerState::Failed {
                            reason: "The server startup stopped".to_owned(),
                        });
                        running.unreported = true;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => continue,
                }
            }
            if running
                .client
                .as_ref()
                .is_some_and(|client| client.is_dead())
            {
                let stable = running
                    .client
                    .as_ref()
                    .is_some_and(|client| client.was_stable());
                let reason = running
                    .client
                    .as_ref()
                    .and_then(|client| client.last_stderr())
                    .unwrap_or_else(|| "The language server disconnected".to_owned());
                running.client = None;
                if stable {
                    running.failures = 0;
                }
                running.failures += 1;
                if running.failures >= RESTART_LIMIT {
                    running.stopped = Some(ServerState::Failed { reason });
                    running.unreported = true;
                } else {
                    let delay = Duration::from_secs(1 << (running.failures - 1));
                    running.retry_at = Some(Instant::now() + delay);
                    let wake = notify.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(delay);
                        wake();
                    });
                }
            }
            if root.host.is_local() {
                let installed = installed_with_recipe(server.command, server.install);
                if installed.is_none() {
                    self.missing.insert(server.command);
                }
                let changed = running.program.as_ref().is_some_and(|previous| {
                    installed
                        .as_ref()
                        .is_some_and(|program| previous != program)
                }) || running
                    .lacking
                    .iter()
                    .any(|program| crate::program::installed(program).is_some());
                let available =
                    running.stopped == Some(ServerState::Missing) && installed.is_some();
                if changed || available {
                    if let Some(client) = running.client.take() {
                        client.shutdown();
                    }
                    running.starting = None;
                    running.stopped = None;
                    running.failures = 0;
                    running.retry_at = None;
                }
            }
            if running.server.is_some_and(|previous| {
                previous.arguments != server.arguments || previous.options != server.options
            }) {
                if let Some(client) = running.client.take() {
                    client.shutdown();
                }
                running.starting = None;
                running.stopped = None;
                running.failures = 0;
            }
            if let Some(client) = &running.client {
                clients.push(client.clone());
                continue;
            }
            if running.starting.is_some()
                || running.stopped.is_some()
                || !root.host.connected()
                || running.retry_at.is_some_and(|at| Instant::now() < at)
            {
                continue;
            }
            running.server = Some(server);
            if root.host.is_local() {
                let lacking = missing_for(server.command);
                self.missing_tools
                    .extend(lacking.iter().filter(|need| need.installable()));
                running.lacking = lacking.iter().map(|need| need.program).collect();
            }
            let root = root.clone();
            let logs = self.logs.clone();
            let wake = notify.clone();
            let (done, starting) = std::sync::mpsc::channel();
            running.starting = Some(starting);
            std::thread::spawn(move || {
                let result = (|| {
                    let program =
                        crate::program::installed_on(&root.host, server.command, server.install)
                            .or_else(|| {
                                root.host
                                    .is_local()
                                    .then(|| crate::program::managed_fallback(server.command))
                                    .flatten()
                            });
                    let Some(program) = program else {
                        let log = log::Log::open(logs.as_deref(), &root.stored(), server.command);
                        log.write(&format!(
                            "{} is not installed on {}",
                            server.command,
                            root.host.name().unwrap_or("the local machine")
                        ));
                        return Ok(None);
                    };
                    Client::start(&root, &program, server, wake.clone(), logs.as_deref())
                        .map(|client| Some((program, client)))
                        .map_err(|error| error.to_string())
                })();
                let _ = done.send(result);
                wake();
            });
        }
        clients
    }

    /// The configured servers' states for a language over a worktree.
    pub fn states(&self, root: &HostLocation, language: Language) -> Vec<ServerStatus> {
        self.wanted(language)
            .into_iter()
            .map(|server| {
                let running = self.running.get(&(root.clone(), server.command));
                let state = running
                    .and_then(|running| running.stopped.clone())
                    .or_else(|| {
                        running.and_then(|running| {
                            running.client.as_ref().map(|client| client.server_state())
                        })
                    })
                    .unwrap_or(ServerState::Starting);
                ServerStatus {
                    command: server.command,
                    state,
                    log: self.log_path(root, server.command),
                }
            })
            .collect()
    }

    /// Persistent logs for every server slot over a worktree.
    pub fn logs_over(&self, root: &HostLocation) -> Vec<(&'static str, PathBuf)> {
        self.running
            .keys()
            .filter(|(started, _)| started == root)
            .filter_map(|(_, command)| Some((*command, self.log_path(root, command)?)))
            .collect()
    }

    /// The log location independent of whether a client is running.
    fn log_path(&self, root: &HostLocation, command: &str) -> Option<PathBuf> {
        self.logs
            .as_ref()
            .map(|directory| directory.join(log::name(&root.stored(), command)))
    }

    /// Takes each terminal failure once per server slot.
    pub fn take_failures(&mut self) -> Vec<ServerStatus> {
        let logs = self.logs.clone();
        self.running
            .iter_mut()
            .filter_map(|((root, command), running)| {
                if !std::mem::take(&mut running.unreported) {
                    return None;
                }
                Some(ServerStatus {
                    command,
                    state: running.stopped.clone()?,
                    log: logs
                        .as_ref()
                        .map(|directory| directory.join(log::name(&root.stored(), command))),
                })
            })
            .collect()
    }

    /// Commands wanted by open documents but absent from program lookup.
    pub fn take_missing(&mut self) -> Vec<Server> {
        let missing = std::mem::take(&mut self.missing);
        missing
            .into_iter()
            .filter_map(|command| self.wanted_server(command))
            .collect()
    }

    /// Installable programs that started servers run in turn but lack.
    pub fn take_missing_tools(&mut self) -> Vec<&'static Need> {
        std::mem::take(&mut self.missing_tools)
            .into_iter()
            .collect()
    }

    /// The first configured server named `command`.
    fn wanted_server(&self, command: &str) -> Option<Server> {
        self.opened
            .keys()
            .filter_map(|name| Language::called(name))
            .flat_map(|language| self.wanted(language))
            .find(|server| server.command == command)
    }

    /// Starts installed servers for each worktree with this language open.
    pub fn reopen(&mut self, language: Language) {
        let roots = self
            .opened
            .get(language.name())
            .cloned()
            .unwrap_or_default();
        for root in roots {
            for server in self.wanted(language) {
                if let Some(running) = self.running.remove(&(root.clone(), server.command))
                    && let Some(client) = running.client
                {
                    client.shutdown();
                }
            }
            self.open(&root, language);
        }
    }

    /// Keeps only roots that still have an open document of each language.
    pub fn retain_opened(&mut self, documents: &[(HostLocation, &'static str)]) {
        self.opened.clear();
        for (root, language) in documents {
            self.opened
                .entry(language)
                .or_default()
                .insert(root.clone());
        }
    }

    /// Drops server slots whose documents or configuration changed.
    pub fn reconcile(&mut self, documents: &[(HostLocation, Language)]) {
        let wanted = documents
            .iter()
            .flat_map(|(root, language)| {
                self.wanted(*language)
                    .into_iter()
                    .map(|server| ((root.clone(), server.command), server))
            })
            .collect::<HashMap<_, _>>();
        self.running.retain(|key, running| {
            let keep = wanted.get(key).is_some_and(|server| {
                running.server.is_none_or(|previous| {
                    previous.arguments == server.arguments && previous.options == server.options
                })
            });
            if !keep && let Some(client) = &running.client {
                client.shutdown();
            }
            keep
        });
        self.retain_opened(
            &documents
                .iter()
                .map(|(root, language)| (root.clone(), language.name()))
                .collect::<Vec<_>>(),
        );
    }

    /// The first installable configured server for `language`.
    pub fn installable(&self, language: Language) -> Option<Server> {
        self.wanted(language)
            .into_iter()
            .find(|server| server.install.is_some())
    }

    /// What the servers for `language` need that the editor cannot install,
    /// when none of them is installed or installable.
    pub fn needs(&self, language: Language) -> Option<&'static str> {
        let wanted = self.wanted(language);
        if wanted.iter().any(|server| {
            server.install.is_some()
                || installed_with_recipe(server.command, server.install).is_some()
        }) {
            return None;
        }
        wanted
            .iter()
            .find_map(|server| crate::install::needs(server.command))
    }

    /// Whether the configured list for `language` includes `command`.
    pub fn uses(&self, language: Language, command: &str) -> bool {
        self.wanted(language)
            .iter()
            .any(|server| server.command == command)
    }

    /// The servers to start for `language`, overrides replacing the language's
    /// own and added servers following them.
    fn wanted(&self, language: Language) -> Vec<Server> {
        let Some(overridden) = self.overrides.get(language.name()) else {
            let mut wanted = language.servers().to_vec();
            self.push_added(language, &mut wanted);
            return wanted;
        };
        overridden.clone()
    }

    /// Appends the servers added for `language` that `wanted` does not already name.
    fn push_added(&self, language: Language, wanted: &mut Vec<Server>) {
        let Some(added) = self.added.get(language.name()) else {
            return;
        };
        for server in added {
            if !wanted.iter().any(|have| have.command == server.command) {
                wanted.push(*server);
            }
        }
    }

    /// Ends every server started for `root`.
    pub fn close(&mut self, root: &HostLocation) {
        for roots in self.opened.values_mut() {
            roots.remove(root);
        }
        self.running.retain(|(started, _), running| {
            if started == root {
                if let Some(client) = &running.client {
                    client.shutdown();
                }
                false
            } else {
                true
            }
        });
    }

    /// Every server running over `root`, whichever language it serves.
    pub fn over(&self, root: &HostLocation) -> Vec<Arc<Client>> {
        self.running
            .iter()
            .filter(|((started, _), _)| started == root)
            .filter_map(|(_, running)| running.client.clone())
            .collect()
    }

    /// Tells every server running over `root` what changed on disk under it.
    pub fn watched(&self, root: &HostLocation, changes: &[(PathBuf, Watched)]) {
        self.running
            .iter()
            .filter(|((started, _), _)| started == root)
            .filter_map(|(_, running)| running.client.as_ref())
            .for_each(|client| client.watched(changes));
    }

    /// Whether any server has said something since this was last asked.
    pub fn take_fresh(&self) -> bool {
        self.running
            .values()
            .filter_map(|running| running.client.as_ref())
            .filter(|client| client.take_fresh())
            .count()
            > 0
    }
}
