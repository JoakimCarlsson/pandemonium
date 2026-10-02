//! The lifecycle and log of one configured language server.

use std::path::PathBuf;

/// The state of a server over one worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerState {
    /// Waiting for the initialization reply, or a scheduled restart.
    Starting,
    /// Initialized with work-done progress in flight.
    Indexing,
    /// Initialized with no work in flight.
    Ready,
    /// Abandoned after a start error or repeated exits.
    Failed {
        /// The start error or last line of stderr.
        reason: String,
    },
    /// The configured command could not be found.
    Missing,
}

impl ServerState {
    /// Orders states from healthy to most in need of attention.
    pub fn severity(&self) -> u8 {
        match self {
            Self::Ready => 0,
            Self::Indexing => 1,
            Self::Starting => 2,
            Self::Missing => 3,
            Self::Failed { .. } => 4,
        }
    }
}

/// A configured server's state and persistent log location.
#[derive(Clone, Debug)]
pub struct ServerStatus {
    /// The command identifying the server.
    pub command: &'static str,
    /// Its current lifecycle state.
    pub state: ServerState,
    /// Its log, including when its client has gone away.
    pub log: Option<PathBuf>,
}

impl ServerStatus {
    /// The failure notice, with a repair instruction for a rustup proxy.
    pub fn failure_notice(&self) -> Option<String> {
        let ServerState::Failed { reason } = &self.state else {
            return None;
        };
        if self.command == "rust-analyzer"
            && ((reason.contains("Unknown binary") && reason.contains("rust-analyzer"))
                || (reason.contains("rust-analyzer") && reason.contains("not installed")))
        {
            return Some("rust-analyzer is not installed for this toolchain. Run: rustup component add rust-analyzer".to_owned());
        }
        Some(format!("{} stopped: {reason}", self.command))
    }
}
