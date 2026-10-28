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
    /// Finds a program on the machine, as a shell would, and returns its path or nothing; needs `process`.
    Which {
        /// The program's name.
        program: String,
    },
    /// Returns the environment of the user's shell, as an object of variables; needs `process`.
    ShellEnv,
    /// Asks GitHub for the latest release of a repository and returns it as a [`Release`]; needs `download`.
    LatestRelease {
        /// The repository, as `owner/name`.
        repository: String,
    },
    /// Fetches a file, following the redirects a release host makes to public addresses, into the extension's own storage, and returns the path it was written to; needs `download`.
    DownloadFile {
        /// The address, `https`.
        url: String,
        /// Where it goes, inside the extension's storage.
        path: String,
        /// The SHA-256 of the file in lowercase hex, which it is checked against when given.
        #[serde(default)]
        sha256: Option<String>,
        /// Whether the file is made a program that can be run.
        #[serde(default)]
        executable: bool,
    },
    /// Lists what is in a folder of the extension's own storage, or its top when no folder is named; needs `download`, since what it holds was fetched.
    StorageList {
        /// The folder, inside the storage.
        #[serde(default)]
        dir: Option<String>,
    },
    /// Removes a file or a folder of the extension's own storage; needs `download`.
    StorageRemove {
        /// What goes, inside the storage.
        path: String,
    },
    /// Tells the reader what the extension is doing, on the extensions page; asks for nothing, and an empty line clears it.
    Status {
        /// The line.
        text: String,
    },
}

/// A file a release holds.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    /// Its name.
    pub name: String,
    /// Where it is fetched from.
    pub url: String,
    /// Its SHA-256 in lowercase hex, when the host says it.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// A release of a repository.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Release {
    /// What the release is called, such as `v1.2.3`.
    pub version: String,
    /// The files it holds.
    pub assets: Vec<ReleaseAsset>,
}

/// One thing in a folder of an extension's storage.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StorageEntry {
    /// Its name.
    pub name: String,
    /// Where it is, as a path on the machine.
    pub path: String,
    /// Whether it is a folder.
    pub dir: bool,
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
