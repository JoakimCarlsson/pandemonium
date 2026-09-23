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
//! replaces that language's list outright.

mod answer;
mod client;
mod encoding;
mod transport;
mod uri;
mod watch;

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use answer::{
    Answer, Calls, CodeAction, Completion, FileEdit, Handle, Lens, Location, NamedLocation,
    Request, Symbol,
};
pub use client::{Asked, Client};
pub use watch::Watched;

use crate::language::{Language, Server};

/// The language servers a window is running.
#[derive(Default)]
pub struct Servers {
    /// One server per worktree and language, by the command that started it.
    running: HashMap<(PathBuf, &'static str), Option<Arc<Client>>>,
    /// The servers to run for a language, where that was overridden by name.
    overrides: HashMap<&'static str, Vec<Server>>,
    /// How a server wakes the window once it has something to say.
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Servers {
    /// Wakes the window through `notify` when a server says something.
    pub fn set_notify(&mut self, notify: Arc<dyn Fn() + Send + Sync>) {
        self.notify = Some(notify);
    }

    /// Runs `overrides` in place of what the languages named by name.
    ///
    /// What a reader configured is not merged with what the editor ships:
    /// naming the servers for a language is how a reader turns one off, and
    /// a list that was added to could not do that.
    pub fn set_overrides(&mut self, overrides: HashMap<&'static str, Vec<Server>>) {
        self.overrides = overrides;
    }

    /// Every server for `language` over `root`, started if not running.
    ///
    /// A server that is not installed is not asked for, and a server that
    /// will not start is remembered as one that will not start: a missing
    /// rust-analyzer is tried once per worktree, not once per file opened.
    pub fn open(&mut self, root: &Path, language: Language) -> Vec<Arc<Client>> {
        let Some(notify) = self.notify.clone() else {
            return Vec::new();
        };
        let wanted = match self.overrides.get(language.name()) {
            Some(overridden) => overridden.clone(),
            None => language.servers().to_vec(),
        };
        wanted
            .iter()
            .filter_map(|server| Some((server, installed(server.command)?)))
            .filter_map(|(server, program)| {
                let notify = notify.clone();
                self.running
                    .entry((root.to_path_buf(), server.command))
                    .or_insert_with(|| {
                        Client::start(root, &program, *server, notify)
                            .ok()
                            .map(Arc::new)
                    })
                    .clone()
            })
            .collect()
    }

    /// Ends every server started for `root`.
    pub fn close(&mut self, root: &Path) {
        self.running.retain(|(started, _), _| started != root);
    }

    /// Every server running over `root`, whichever language it serves.
    pub fn over(&self, root: &Path) -> Vec<Arc<Client>> {
        self.running
            .iter()
            .filter(|((started, _), _)| started == root)
            .filter_map(|(_, client)| client.clone())
            .collect()
    }

    /// Tells every server running over `root` what changed on disk under it.
    pub fn watched(&self, root: &Path, changes: &[(PathBuf, Watched)]) {
        self.running
            .iter()
            .filter(|((started, _), _)| started == root)
            .filter_map(|(_, client)| client.as_ref())
            .for_each(|client| client.watched(changes));
    }

    /// Whether any server has said something since this was last asked.
    pub fn take_fresh(&self) -> bool {
        self.running
            .values()
            .flatten()
            .filter(|client| client.take_fresh())
            .count()
            > 0
    }
}

/// The directories a server is looked for in besides the path.
///
/// A window started from a desktop session inherits the path that session
/// was given, which is not the one a shell has: rustup, go and npm each put
/// their programs somewhere that only a shell profile ever hears about. A
/// server the reader has installed is a server the editor runs, whether or
/// not the session was told where it lives.
const TOOL_DIRECTORIES: [&str; 8] = [
    ".cargo/bin",
    ".local/bin",
    "go/bin",
    ".bun/bin",
    ".deno/bin",
    ".npm-global/bin",
    ".volta/bin",
    ".local/share/fnm/aliases/default/bin",
];

/// Where `command` is installed, on the path or in the usual places beside it.
///
/// Nothing is started to find out: a server that is nowhere is one the
/// reader does not have, and the editor does not try to run it.
fn installed(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    let home = env::var_os("HOME").map(PathBuf::from);

    env::split_paths(&path)
        .chain(
            TOOL_DIRECTORIES
                .iter()
                .filter_map(|directory| Some(home.as_ref()?.join(directory))),
        )
        .chain([
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
        ])
        .map(|directory| directory.join(command))
        .find(|program| program.is_file())
}
