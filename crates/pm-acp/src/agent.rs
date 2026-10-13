//! The agents the app ships, and how each is started.

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// The program that runs a published package without installing it first.
const RUNNER: &str = "npx";

/// Where an agent's program comes from when the reader does not have it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    /// A published package that the runner fetches the first time the agent
    /// starts, so the agent is startable wherever the runner is.
    Package(&'static str),
    /// A program that an installer script puts on the machine.
    Installer {
        /// The script, as a shell pipeline that installs the program.
        script: &'static str,
    },
}

/// One agent the app knows how to run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Agent {
    /// The name the app keeps the agent under.
    pub id: &'static str,
    /// The name a row calls it.
    pub name: &'static str,
    /// The program to run when it is installed.
    pub program: &'static str,
    /// The arguments that put the program into protocol mode.
    pub arguments: &'static [&'static str],
    /// Where the program comes from when it is not installed.
    pub source: Source,
}

/// Every agent the app ships, in the order onboarding lists them.
pub const AGENTS: [Agent; 3] = [
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
        id: "grok",
        name: "Grok Build",
        program: "grok",
        arguments: &["agent", "stdio"],
        source: Source::Installer {
            script: "curl -fsSL https://x.ai/cli/install.sh | bash",
        },
    },
];

/// The directories a program is looked for in besides the path.
///
/// A window started from a desktop session inherits the path that session
/// was given, which is not the one a shell has: npm, bun and cargo each put
/// their programs somewhere only a shell profile hears about.
const TOOL_DIRECTORIES: [&str; 8] = [
    ".local/bin",
    ".grok/bin",
    ".bun/bin",
    ".deno/bin",
    ".npm-global/bin",
    "AppData/Roaming/npm",
    ".volta/bin",
    ".cargo/bin",
];

/// Where `program` is installed, on the path or in the usual places beside it.
pub(crate) fn installed(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    let home = env::home_dir();
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

impl Agent {
    /// Whether the agent can be started on this machine: its program is
    /// installed, or it is a package and the runner is.
    pub fn startable(self) -> bool {
        installed(self.program).is_some()
            || (matches!(self.source, Source::Package(_)) && installed(RUNNER).is_some())
    }

    /// Whether starting the agent has to fetch it first, which takes a while.
    pub(crate) fn fetches(self) -> bool {
        installed(self.program).is_none() && matches!(self.source, Source::Package(_))
    }

    /// The command that starts the agent in protocol mode, with `extra`
    /// arguments after the ones that put it there.
    pub(crate) fn command(self, extra: &[String]) -> Command {
        let mut command = match (installed(self.program), self.source) {
            (None, Source::Package(package)) => {
                let mut command = Command::new(installed(RUNNER).unwrap_or_else(|| RUNNER.into()));
                command.arg("--yes").arg(package);
                command
            }
            (program, _) => Command::new(program.unwrap_or_else(|| self.program.into())),
        };
        command.args(self.arguments).args(extra);
        command
    }
}
