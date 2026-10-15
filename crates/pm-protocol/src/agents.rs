//! The records the server keeps its agents in: the ones the app ships, and the agent a record describes.

use crate::AgentServer;

/// The agents the app ships, which are in the list of a server that has none saved.
pub fn builtin_agents() -> Vec<AgentServer> {
    pm_acp::AGENTS
        .iter()
        .map(|agent| AgentServer {
            id: agent.id.to_owned(),
            name: agent.name.to_owned(),
            command: pm_acp::command_line(
                std::iter::once(agent.program).chain(agent.arguments.iter().copied()),
            ),
            env: agent
                .env
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            package: match agent.source {
                pm_acp::Source::Package(package) => Some(package.to_owned()),
                _ => None,
            },
            installer: match agent.source {
                pm_acp::Source::Installer { script } => Some(script.to_owned()),
                _ => None,
            },
            home: agent.home.map(str::to_owned),
        })
        .collect()
}

impl AgentServer {
    /// The agent the record describes, or nothing when its command is empty.
    pub fn agent(&self) -> Option<pm_acp::Agent> {
        pm_acp::Agent::define(pm_acp::Definition {
            id: &self.id,
            name: &self.name,
            command: &self.command,
            env: &self.env,
            package: self.package.as_deref(),
            installer: self.installer.as_deref(),
            home: self.home.as_deref(),
        })
    }
}
