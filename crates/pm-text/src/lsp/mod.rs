//! Language servers: starting them, and keeping one per worktree.
//!
//! A server belongs to a worktree and a language, not to a file: every Rust
//! file of one checkout is served by the one rust-analyzer that was started
//! for it. [`Servers`] is that seam — the only place a server process is
//! started, found or ended.

mod answer;
mod client;
mod transport;
mod uri;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use answer::{Answer, CodeAction, Completion, FileEdit, Location, Request, Symbol};
pub use client::{Asked, Client};

use crate::language::Language;

/// The language servers a window is running.
#[derive(Default)]
pub struct Servers {
    /// One server per worktree and language, by the command that started it.
    running: HashMap<(PathBuf, &'static str), Option<Arc<Client>>>,
    /// How a server wakes the window once it has something to say.
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Servers {
    /// Wakes the window through `notify` when a server says something.
    pub fn set_notify(&mut self, notify: Arc<dyn Fn() + Send + Sync>) {
        self.notify = Some(notify);
    }

    /// The server for `language` over `root`, started if it is not running.
    ///
    /// A server that will not start is remembered as one that will not
    /// start: a missing rust-analyzer is asked for once per worktree, not
    /// once per file opened in it.
    pub fn open(&mut self, root: &Path, language: Language) -> Option<Arc<Client>> {
        let server = language.server()?;
        let notify = self.notify.clone()?;
        self.running
            .entry((root.to_path_buf(), server.command))
            .or_insert_with(|| Client::start(root, server, notify).ok().map(Arc::new))
            .clone()
    }

    /// Ends every server started for `root`.
    pub fn close(&mut self, root: &Path) {
        self.running.retain(|(started, _), _| started != root);
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
