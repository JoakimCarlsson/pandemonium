//! What a client on the same machine needs to reach the server.

use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

/// Where a running server listens and the token it wants.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Credential {
    /// The port it listens on.
    pub port: u16,
    /// The token a client sends in its first message.
    pub token: String,
    /// The process that runs it.
    pub pid: u32,
    /// The program it runs, as it was when it started; missing from servers that predate it.
    #[serde(default)]
    pub build: Option<Build>,
}

/// A program file as it was at one moment, so that a rebuilt program can be told from the one still running.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Build {
    /// The path of the program file.
    pub program: String,
    /// When the program file was last written, in milliseconds since the epoch.
    pub modified: u64,
}

impl Build {
    /// The program this process runs, as its file is now.
    pub fn current() -> Option<Self> {
        let program = std::env::current_exe().ok()?;
        let modified = std::fs::metadata(&program)
            .and_then(|data| data.modified())
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis() as u64;
        Some(Self {
            program: program.to_string_lossy().into_owned(),
            modified,
        })
    }

    /// Whether `self` is an older or newer build of the same program file as `other`.
    pub fn superseded_by(&self, other: &Self) -> bool {
        self.program == other.program && self.modified != other.modified
    }
}

/// Where a server is and the token that lets a client in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    /// The address, as `host:port`.
    pub address: String,
    /// The token to send in the first message.
    pub token: String,
}
