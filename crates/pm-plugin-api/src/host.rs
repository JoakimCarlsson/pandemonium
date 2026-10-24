//! What an extension's code may ask the host to do for it.

use serde::{Deserialize, Serialize};

/// What the module asks the host for; every one is gated by a capability the reader grants.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum HostCall {
    /// Runs a program and waits for it; needs `process`.
    Process {
        /// The program.
        program: String,
        /// What it is started with.
        #[serde(default)]
        args: Vec<String>,
        /// The folder it runs in, inside the project when it is relative; the project's own when nothing.
        #[serde(default)]
        cwd: Option<String>,
    },
    /// Fetches a file and returns its text; needs `download`.
    Download {
        /// The address, `http` or `https`.
        url: String,
    },
    /// Reads a text file of the project; needs `read-project`.
    ReadFile {
        /// The path inside the project.
        path: String,
    },
}

/// What a finished program left.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProcessOutput {
    /// Its exit status, or `-1` when it was ended by a signal.
    pub status: i32,
    /// What it wrote to its standard output.
    pub stdout: String,
    /// What it wrote to its standard error.
    pub stderr: String,
}

/// The host's answer to a [`HostCall`]: what it came to, or why not.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostReply {
    /// It was done; a program's [`ProcessOutput`], or the text of a file.
    Ok(serde_json::Value),
    /// It was refused or failed, with the reason.
    Error(String),
}

/// Whoever does what a module asks, after deciding whether it may.
pub trait Host: Send + Sync {
    /// Does `call`, or says why it cannot be done.
    fn call(&self, call: HostCall) -> Result<serde_json::Value, String>;
}
