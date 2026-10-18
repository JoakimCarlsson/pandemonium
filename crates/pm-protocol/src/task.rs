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
}
