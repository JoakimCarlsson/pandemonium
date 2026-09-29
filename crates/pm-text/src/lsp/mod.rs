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

mod answer;
mod capabilities;
mod client;
mod encoding;
mod log;
mod outbox;
mod progress;
mod rpc;
mod sync;
mod uri;
mod watch;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use answer::{
    Answer, Calls, CodeAction, Completion, FileEdit, Handle, Lens, Location, NamedLocation,
    Request, Symbol, WorkspaceChange,
};
pub use client::{Asked, Client};
pub use progress::Progress;
pub use watch::Watched;

/// How many consecutive exits are allowed before a server is abandoned.
const RESTART_LIMIT: u32 = 4;

/// One server slot and its bounded restart schedule.
#[derive(Default)]
struct Running {
    /// The client currently occupying the slot.
    client: Option<Arc<Client>>,
    /// How often a client in this slot has died.
    failures: u32,
    /// The earliest time another start may be tried.
    retry_at: Option<Instant>,
}

use crate::language::{Language, Server};
use crate::program::installed;

/// The language servers a window is running.
#[derive(Default)]
pub struct Servers {
    /// One server per worktree and language, by the command that started it.
    running: HashMap<(PathBuf, &'static str), Running>,
    /// The servers to run for a language, where that was overridden by name.
    overrides: HashMap<&'static str, Vec<Server>>,
    /// The servers to run for a language after the ones it names.
    added: HashMap<&'static str, Vec<Server>>,
    /// How a server wakes the window once it has something to say.
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Missing commands observed while opening documents.
    missing: HashSet<&'static str>,
    /// Worktrees with an open document of each language.
    opened: HashMap<&'static str, HashSet<PathBuf>>,
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
    }

    /// Runs `added` for the languages they name, after the servers those
    /// languages name.
    ///
    /// An override still replaces the list outright: servers added for a
    /// language that was also overridden are not run.
    pub fn set_added(&mut self, added: HashMap<&'static str, Vec<Server>>) {
        self.added = added;
    }

    /// Every server for `language` over `root`, started if not running.
    ///
    /// A server that is not installed is not asked for, and a server that
    /// will not start is remembered as one that will not start: a missing
    /// rust-analyzer is tried once per worktree, not once per file opened.
    pub fn open(&mut self, root: &Path, language: Language) -> Vec<Arc<Client>> {
        self.opened
            .entry(language.name())
            .or_default()
            .insert(root.to_path_buf());
        let Some(notify) = self.notify.clone() else {
            return Vec::new();
        };
        let wanted = self.wanted(language);
        let logs = self.logs.clone();
        wanted
            .iter()
            .filter_map(|server| match installed(server.command) {
                Some(program) => Some((server, program)),
                None => {
                    self.missing.insert(server.command);
                    None
                }
            })
            .filter_map(|(server, program)| {
                let notify = notify.clone();
                let running = self
                    .running
                    .entry((root.to_path_buf(), server.command))
                    .or_default();
                if running
                    .client
                    .as_ref()
                    .is_some_and(|client| client.is_dead())
                {
                    if running
                        .client
                        .as_ref()
                        .is_some_and(|client| client.was_stable())
                    {
                        running.failures = 0;
                    }
                    running.client = None;
                    running.failures += 1;
                    if running.failures < RESTART_LIMIT {
                        let delay = Duration::from_secs(1 << (running.failures - 1));
                        running.retry_at = Some(Instant::now() + delay);
                        std::thread::spawn(move || {
                            std::thread::sleep(delay);
                            notify();
                        });
                    }
                    return None;
                }
                if running.failures >= RESTART_LIMIT
                    || running.retry_at.is_some_and(|at| Instant::now() < at)
                {
                    return None;
                }
                if running.client.is_none() {
                    running.client =
                        Client::start(root, &program, *server, notify, logs.as_deref())
                            .ok()
                            .map(Arc::new);
                    if running.client.is_none() {
                        running.failures = RESTART_LIMIT;
                    }
                }
                running.client.clone()
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
            self.open(&root, language);
        }
    }

    /// Keeps only roots that still have an open document of each language.
    pub fn retain_opened(&mut self, documents: &[(PathBuf, &'static str)]) {
        self.opened.clear();
        for (root, language) in documents {
            self.opened
                .entry(language)
                .or_default()
                .insert(root.clone());
        }
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
        if wanted
            .iter()
            .any(|server| server.install.is_some() || installed(server.command).is_some())
        {
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
    pub fn close(&mut self, root: &Path) {
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
    pub fn over(&self, root: &Path) -> Vec<Arc<Client>> {
        self.running
            .iter()
            .filter(|((started, _), _)| started == root)
            .filter_map(|(_, running)| running.client.clone())
            .collect()
    }

    /// Tells every server running over `root` what changed on disk under it.
    pub fn watched(&self, root: &Path, changes: &[(PathBuf, Watched)]) {
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
