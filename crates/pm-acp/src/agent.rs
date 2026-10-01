//! The agents the editor can conduct, and how each of them is started.
//!
//! An agent is a program that speaks the protocol on its standard input and
//! output. Some speak it themselves and some are spoken for by an adapter,
//! but that difference stops here: above this module an agent is a name, a
//! command and nothing else.

use pm_host::Command;
use std::path::PathBuf;
use std::sync::RwLock;

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
    /// The environment the program is started with, over the one it inherits.
    pub env: &'static [(&'static str, &'static str)],
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
    /// A program named by its command. Nothing is fetched, and nowhere is
    /// named to install it from.
    Command,
}

impl Source {
    /// What the picker writes beside an agent that is not installed yet.
    #[must_use]
    pub fn hint(self) -> String {
        match self {
            Self::Package(package) => format!("{package} (fetched on first run)"),
            Self::Installer(origin) => format!("install from {origin}"),
            Self::Command => "not installed".to_owned(),
        }
    }
}

/// No environment of an agent's own.
const NO_ENVIRONMENT: &[(&str, &str)] = &[];

/// Every agent the editor ships knowing about.
///
/// What a launch offers is [`agents`]: these, with the reader's own laid
/// over them. A reader who has none of them installed still sees all of
/// them: an agent that is missing is fetched by [`RUNNER`] the first time it
/// is started, which is how most of these are meant to be run. The few that
/// ship with an application of their own are named by where they are had
/// from instead.
pub const AGENTS: [Agent; 6] = [
    Agent {
        id: "claude-code",
        name: "Claude Code",
        program: "claude-agent-acp",
        arguments: &[],
        env: NO_ENVIRONMENT,
        source: Source::Package("@agentclientprotocol/claude-agent-acp"),
    },
    Agent {
        id: "codex",
        name: "Codex",
        program: "codex-acp",
        arguments: &[],
        env: NO_ENVIRONMENT,
        source: Source::Package("@agentclientprotocol/codex-acp"),
    },
    Agent {
        id: "gemini",
        name: "Gemini",
        program: "gemini",
        arguments: &["--experimental-acp"],
        env: NO_ENVIRONMENT,
        source: Source::Package("@google/gemini-cli"),
    },
    Agent {
        id: "copilot",
        name: "GitHub Copilot",
        program: "copilot",
        arguments: &["--acp"],
        env: NO_ENVIRONMENT,
        source: Source::Package("@github/copilot"),
    },
    Agent {
        id: "cursor",
        name: "Cursor",
        program: "agent",
        arguments: &["acp"],
        env: NO_ENVIRONMENT,
        source: Source::Installer("cursor.com"),
    },
    Agent {
        id: "grok",
        name: "Grok Build",
        program: "grok",
        arguments: &["agent", "stdio"],
        env: NO_ENVIRONMENT,
        source: Source::Installer("x.ai"),
    },
];

/// The agents a launch offers, once [`install`] has laid the reader's over
/// the ones shipped.
static OFFERED: RwLock<&'static [Agent]> = RwLock::new(&[]);

/// Offers the shipped agents with `custom` laid over them.
///
/// A custom agent whose id matches a shipped one takes its place. One whose
/// id is new is offered after the shipped ones. The list lives as long as
/// the process does, so a session still holding an agent from the last list
/// never sees it go.
pub fn install(custom: Vec<Agent>) {
    let mut offered = AGENTS.to_vec();
    for agent in custom {
        match offered.iter().position(|shipped| shipped.id == agent.id) {
            Some(place) => offered[place] = agent,
            None => offered.push(agent),
        }
    }
    let offered = offered.leak();
    if let Ok(mut agents) = OFFERED.write() {
        *agents = offered;
    }
}

/// The agents a launch offers: the shipped ones, then the reader's own.
#[must_use]
pub fn agents() -> &'static [Agent] {
    OFFERED
        .read()
        .ok()
        .map(|agents| *agents)
        .filter(|agents| !agents.is_empty())
        .unwrap_or(&AGENTS)
}

impl Agent {
    /// The agent `id` names, if the editor knows one by that name.
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        agents().iter().copied().find(|agent| agent.id == id)
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
        let mut command = match (installed(self.program), self.source) {
            (None, Source::Package(package)) => {
                let mut command = pm_host::Host::local()
                    .command(installed(RUNNER).unwrap_or_else(|| PathBuf::from(RUNNER)));
                command.arg("--yes").arg(package);
                command
            }
            (program, _) => pm_host::Host::local()
                .command(program.unwrap_or_else(|| PathBuf::from(self.program))),
        };
        command.args(self.arguments);
        command.envs(self.env.iter().copied());
        command
    }
}

/// Finds the program on the local execution machine.
fn installed(program: &str) -> Option<PathBuf> {
    pm_host::Host::local().which(program)
}
