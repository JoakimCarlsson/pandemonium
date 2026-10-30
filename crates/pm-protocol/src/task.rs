//! What is told about the toolchains a project is built and run with.

use serde::{Deserialize, Serialize};

/// One toolchain installed on the server's machine, or kept by a project.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolchainInfo {
    /// The language it is for, by family: `rust`, `javascript`, `python` or `go`.
    pub language: String,
    /// What `select_toolchain` chooses it by; `system` is the one the machine's own path finds.
    pub id: String,
    /// What a reader calls it.
    pub name: String,
    /// Which version it is, or where it is.
    pub detail: String,
    /// Whether the project's language is set to use it; the system's counts as chosen when the project chose none.
    pub selected: bool,
    /// What a reader calls the language, as its manifest says; empty when it says none.
    #[serde(default)]
    pub language_name: String,
}

/// One task that can run a place of a file, as a client lists them for the reader to pick from.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunnableTask {
    /// What it is called; `run_runnable` names it by this.
    pub name: String,
    /// What it types into the shell.
    pub command: String,
}
