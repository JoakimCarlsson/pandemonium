//! The agents the app ships, and how each is started.

use std::process::Command;
use std::sync::Mutex;

use pm_tools::{installed, path_beside};

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
    /// A program the reader named, which the app neither fetches nor installs.
    Manual,
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
    /// The environment it is started with, over the one it inherits.
    pub env: &'static [(&'static str, &'static str)],
    /// The variable that moves its config and login to another folder, when it has one.
    pub home: Option<&'static str>,
}

/// Every agent the app ships, in the order onboarding lists them.
pub const AGENTS: [Agent; 3] = [
    Agent {
        id: "claude-code",
        name: "Claude Code",
        program: "claude-agent-acp",
        arguments: &[],
        source: Source::Package("@agentclientprotocol/claude-agent-acp"),
        env: &[],
        home: Some("CLAUDE_CONFIG_DIR"),
    },
    Agent {
        id: "codex",
        name: "Codex",
        program: "codex-acp",
        arguments: &[],
        source: Source::Package("@agentclientprotocol/codex-acp"),
        env: &[],
        home: Some("CODEX_HOME"),
    },
    Agent {
        id: "grok",
        name: "Grok Build",
        program: "grok",
        arguments: &["agent", "stdio"],
        source: Source::Installer {
            script: "curl -fsSL https://x.ai/cli/install.sh | bash",
        },
        env: &[],
        home: Some("GROK_HOME"),
    },
];

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
                let runner = installed(RUNNER);
                let mut command = Command::new(runner.clone().unwrap_or_else(|| RUNNER.into()));
                if let Some(runner) = runner.as_deref() {
                    command.env("PATH", path_beside(runner));
                }
                command.arg("--yes").arg(package);
                command
            }
            (program, _) => Command::new(program.unwrap_or_else(|| self.program.into())),
        };
        command
            .args(self.arguments)
            .args(extra)
            .envs(self.env.iter().copied());
        if let Some((name, home)) = self.environment() {
            command.env(name, home);
        }
        command
    }
}

/// The agents made from records so far, by what defines each, so that asking again for one does not hold more memory.
static DEFINED: Mutex<Vec<(String, Agent)>> = Mutex::new(Vec::new());

/// Leaks `text` into a string that lives as long as the program.
fn kept(text: &str) -> &'static str {
    Box::leak(text.to_owned().into_boxed_str())
}

/// What an agent is defined by, as the record the servers keep it in.
#[derive(Clone, Copy, Debug)]
pub struct Definition<'a> {
    /// The name the app keeps the agent under.
    pub id: &'a str,
    /// The name a row calls it.
    pub name: &'a str,
    /// The command that starts it in protocol mode, program and arguments.
    pub command: &'a str,
    /// The environment it is started with, over the one it inherits.
    pub env: &'a [(String, String)],
    /// The package the runner fetches when the program is not installed.
    pub package: Option<&'a str>,
    /// The shell pipeline that installs the program.
    pub installer: Option<&'a str>,
    /// The variable that moves its config and login to another folder.
    pub home: Option<&'a str>,
}

impl Agent {
    /// The agent `definition` describes, or nothing when its command is empty.
    pub fn define(definition: Definition) -> Option<Agent> {
        let mut words = crate::mcp::command_words(definition.command).into_iter();
        let program = words.next()?;
        let arguments: Vec<String> = words.collect();
        let key = format!(
            "{}\0{}\0{}\0{:?}\0{:?}\0{:?}\0{:?}",
            definition.id,
            definition.name,
            definition.command,
            definition.env,
            definition.package,
            definition.installer,
            definition.home
        );
        let mut held = DEFINED.lock().unwrap_or_else(|held| held.into_inner());
        if let Some((_, agent)) = held.iter().find(|(held, _)| *held == key) {
            return Some(*agent);
        }
        let source = match (definition.package, definition.installer) {
            (Some(package), _) => Source::Package(kept(package)),
            (None, Some(script)) => Source::Installer {
                script: kept(script),
            },
            (None, None) => Source::Manual,
        };
        let arguments: Vec<&'static str> = arguments.iter().map(|word| kept(word)).collect();
        let env: Vec<(&'static str, &'static str)> = definition
            .env
            .iter()
            .map(|(name, value)| (kept(name), kept(value)))
            .collect();
        let agent = Agent {
            id: kept(definition.id),
            name: kept(definition.name),
            program: kept(&program),
            arguments: Vec::leak(arguments),
            source,
            env: Vec::leak(env),
            home: definition
                .home
                .or_else(|| {
                    crate::product::Product::of(&format!(
                        "{} {}",
                        definition.command,
                        definition.package.unwrap_or_default()
                    ))
                    .map(|product| product.home_variable())
                })
                .map(kept),
        };
        held.retain(|(_, older)| older.id != definition.id);
        held.push((key, agent));
        Some(agent)
    }
}
