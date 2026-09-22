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
    /// Where the program comes from when it is not installed.
    pub source: Source,
}

/// Where an agent's program comes from when the reader does not have it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Source {
    /// A published package, which [`RUNNER`] fetches the first time the
    /// agent is started.
    Package(&'static str),
    /// A program the reader installs themselves, named by where it is had
    /// from. An agent that comes this way is only started once it is there.
    Installer(&'static str),
}

impl Source {
    /// What the picker writes beside an agent that is not installed yet.
    #[must_use]
    pub fn hint(self) -> String {
        match self {
            Self::Package(package) => format!("{package} (fetched on first run)"),
            Self::Installer(origin) => format!("install from {origin}"),
        }
    }
}

/// Every agent the editor ships knowing about.
///
/// A reader who has none of them installed still sees all of them: an agent
/// that is missing is fetched by [`RUNNER`] the first time it is started,
/// which is how most of these are meant to be run. The few that ship with an
/// application of their own are named by where they are had from instead.
pub const AGENTS: [Agent; 5] = [
    Agent {
        id: "claude-code",
        name: "Claude Code",
        program: "claude-agent-acp",
        arguments: &[],
        source: Source::Package("@agentclientprotocol/claude-agent-acp"),
    },
    Agent {
        id: "codex",
        name: "Codex",
        program: "codex-acp",
        arguments: &[],
        source: Source::Package("@agentclientprotocol/codex-acp"),
    },
    Agent {
        id: "gemini",
        name: "Gemini",
        program: "gemini",
        arguments: &["--experimental-acp"],
        source: Source::Package("@google/gemini-cli"),
    },
    Agent {
        id: "copilot",
        name: "GitHub Copilot",
        program: "copilot",
        arguments: &["--acp"],
        source: Source::Package("@github/copilot"),
    },
    Agent {
        id: "cursor",
        name: "Cursor",
        program: "agent",
        arguments: &["acp"],
        source: Source::Installer("cursor.com"),
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

    /// Whether the agent can be started on this machine.
    ///
    /// An agent [`RUNNER`] can fetch is always startable; one that comes
    /// from an installer is startable once the reader has installed it.
    #[must_use]
    pub fn startable(self) -> bool {
        matches!(self.source, Source::Package(_)) || self.installed()
    }

    /// The command that starts this agent, installed or not.
    ///
    /// Neither the pipes nor the working directory are set here: what the
    /// agent is run in is the session's business, and this says only what is
    /// run.
    #[must_use]
    pub fn command(self) -> Command {
        match (installed(self.program), self.source) {
            (None, Source::Package(package)) => {
                let mut command = Command::new(RUNNER);
                command.arg("--yes").arg(package).args(self.arguments);
                command
            }
            (program, _) => {
                let mut command =
                    Command::new(program.unwrap_or_else(|| PathBuf::from(self.program)));
                command.args(self.arguments);
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
