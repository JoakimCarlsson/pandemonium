//! The agents the app ships, and where the harness that runs them is.
//!
//! Every agent is one provider of the harness, which holds the conversation
//! and runs the tools; the record here says which provider and under which
//! name the app keeps it.

use std::path::PathBuf;
use std::process::Command;

use pm_tools::{installed, path_beside};

use crate::product::Product;

/// The program that is the harness.
const HARNESS: &str = "crab";

/// The variable that points at a harness the reader built themselves.
const HARNESS_PATH: &str = "PANDEMONIUM_HARNESS";

/// One agent the app knows how to run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Agent {
    /// The name the app keeps the agent under.
    pub id: &'static str,
    /// The name a row calls it.
    pub name: &'static str,
    /// The product whose files and customizations the app knows.
    pub product: Product,
    /// The provider the harness reaches the model through.
    pub provider: &'static str,
}

/// Every agent the app ships, in the order the workspace lists them.
pub const AGENTS: [Agent; 2] = [
    Agent {
        id: "claude-code",
        name: "Claude",
        product: Product::Claude,
        provider: "claude",
    },
    Agent {
        id: "codex",
        name: "Codex",
        product: Product::Codex,
        provider: "codex",
    },
];

/// Where the harness is: the one `PANDEMONIUM_HARNESS` names, the one beside this program, or the first on the path.
pub fn harness_program() -> Option<PathBuf> {
    std::env::var_os(HARNESS_PATH)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            let beside = std::env::current_exe().ok()?.parent()?.join(HARNESS);
            beside.is_file().then_some(beside)
        })
        .or_else(|| installed(HARNESS))
}

impl Agent {
    /// Whether the agent can be started on this machine: the harness is installed.
    pub fn startable(self) -> bool {
        harness_program().is_some()
    }

    /// The command that starts the harness in serving mode.
    pub(crate) fn command(self) -> Option<Command> {
        let program = harness_program()?;
        let mut command = Command::new(&program);
        command.arg("serve").env("PATH", path_beside(&program));
        Some(command)
    }
}
