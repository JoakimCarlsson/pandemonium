//! The agents the editor can conduct, and how each of them is started.
//!
//! An agent is a program that speaks the protocol on its standard input and
//! output. Some speak it themselves and some are spoken for by an adapter,
//! but that difference stops here: above this module an agent is a name, a
//! command and nothing else.

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// The program that runs a published package without installing it first.
const RUNNER: &str = "npx";

/// One agent the editor knows how to run.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Agent {
    /// What the editor calls this agent where a name has to be written down.
    pub id: &'static str,
    /// What a tab, a menu and a session list call it.
    pub name: &'static str,
    /// The program to run when it is installed.
    pub program: &'static str,
    /// The arguments that put the program into protocol mode.
    pub arguments: &'static [&'static str],
    /// The package [`RUNNER`] fetches when the program is not installed.
    pub package: &'static str,
}

/// Every agent the editor ships knowing about.
///
/// A reader who has none of them installed still sees all of them: an agent
/// that is missing is fetched by [`RUNNER`] the first time it is started,
/// which is how these are meant to be run.
pub const AGENTS: [Agent; 3] = [
    Agent {
        id: "claude-code",
        name: "Claude Code",
        program: "claude-agent-acp",
        arguments: &[],
        package: "@agentclientprotocol/claude-agent-acp",
    },
    Agent {
        id: "codex",
        name: "Codex",
        program: "codex-acp",
        arguments: &[],
        package: "@agentclientprotocol/codex-acp",
    },
    Agent {
        id: "gemini",
        name: "Gemini",
        program: "gemini",
        arguments: &["--experimental-acp"],
        package: "@google/gemini-cli",
    },
];

impl Agent {
    /// The agent `id` names, if the editor knows one by that name.
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        AGENTS.into_iter().find(|agent| agent.id == id)
    }

    /// Whether the agent's own program is installed.
    #[must_use]
    pub fn installed(self) -> bool {
        installed(self.program).is_some()
    }

    /// The command that starts this agent, installed or not.
    ///
    /// Neither the pipes nor the working directory are set here: what the
    /// agent is run in is the session's business, and this says only what is
    /// run.
    #[must_use]
    pub fn command(self) -> Command {
        match installed(self.program) {
            Some(program) => {
                let mut command = Command::new(program);
                command.args(self.arguments);
                command
            }
            None => {
                let mut command = Command::new(RUNNER);
                command.arg("--yes").arg(self.package).args(self.arguments);
                command
            }
        }
    }
}

/// The directories a program is looked for in besides the path.
///
/// A window started from a desktop session inherits the path that session
/// was given, which is not the one a shell has: npm, bun and cargo each put
/// their programs somewhere that only a shell profile ever hears about. An
/// agent the reader has installed is the one the editor runs, whether or not
/// the session was told where it lives.
const TOOL_DIRECTORIES: [&str; 6] = [
    ".local/bin",
    ".bun/bin",
    ".deno/bin",
    ".npm-global/bin",
    ".volta/bin",
    ".cargo/bin",
];

/// Where `program` is installed, on the path or in the usual places beside it.
fn installed(program: &str) -> Option<PathBuf> {
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
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}
