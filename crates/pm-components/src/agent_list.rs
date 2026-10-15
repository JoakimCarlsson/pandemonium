//! The agents a workspace can be started with: the ones the server lists, which start as the ones the app ships.

use pm_icon::IconName;
use pm_protocol::AgentServer;

use crate::state::WorkspaceState;

/// How an agent is shown: its glyph and its name.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentLook {
    /// The glyph on its chip.
    pub icon: IconName,
    /// What it is called.
    pub name: String,
}

/// The glyph an agent is drawn with: the one of a product the app knows, or a terminal for any other.
pub fn agent_icon(agent: &AgentServer) -> IconName {
    match pm_acp::Product::of(&format!(
        "{} {}",
        agent.command,
        agent.package.as_deref().unwrap_or_default()
    )) {
        Some(pm_acp::Product::Claude) => IconName::Sparkles,
        Some(pm_acp::Product::Grok) => IconName::Zap,
        _ => IconName::Terminal,
    }
}

/// Rebuilds what is kept per agent for a new list: each agent keeps what it had, found by where `origin` says it was among the `before` agents, and a new agent starts with nothing.
fn carry<T: Default>(held: &mut Vec<T>, origin: &[Option<usize>], before: usize) {
    let mut old = std::mem::take(held);
    let aligned = old.len() == before;
    *held = origin
        .iter()
        .map(|from| match from {
            Some(from) if aligned => std::mem::take(&mut old[*from]),
            _ => T::default(),
        })
        .collect();
}

impl WorkspaceState {
    /// How many agents are offered for new work.
    pub fn agent_slots(&self) -> usize {
        self.settings_servers.agents.len()
    }

    /// The positions of the agents offered for new work, in the order the chips show them.
    pub fn offered_agents(&self) -> Vec<usize> {
        (0..self.agent_slots()).collect()
    }

    /// The agent at `index`: one offered, or, past those, one taken out of the list that sessions still run.
    fn agent_record(&self, index: usize) -> Option<&AgentServer> {
        self.settings_servers
            .agents
            .iter()
            .chain(&self.settings_servers.retired)
            .nth(index)
    }

    /// The id the servers know the agent at `index` by.
    pub fn agent_id_at(&self, index: usize) -> Option<String> {
        self.agent_record(index).map(|agent| agent.id.clone())
    }

    /// The position of the agent the servers know by `id`, offered or retired.
    pub fn agent_position(&self, id: &str) -> Option<usize> {
        self.settings_servers
            .agents
            .iter()
            .chain(&self.settings_servers.retired)
            .position(|agent| agent.id == id)
    }

    /// The position of the agent `id`, retiring it when the list does not have it, so a session it ran can still be shown.
    pub(crate) fn agent_position_or_retire(&mut self, id: &str) -> usize {
        if let Some(position) = self.agent_position(id) {
            return position;
        }
        let record = pm_protocol::builtin_agents()
            .into_iter()
            .find(|agent| agent.id == id)
            .unwrap_or_else(|| AgentServer {
                id: id.to_owned(),
                name: id.to_owned(),
                command: String::new(),
                env: Vec::new(),
                package: None,
                installer: None,
                home: None,
            });
        self.settings_servers.retired.push(record);
        self.agent_slots() + self.settings_servers.retired.len() - 1
    }

    /// Whether the agent at `index` can keep more than one login, which every agent can.
    pub fn agent_has_accounts(&self, index: usize) -> bool {
        self.agent_record(index).is_some()
    }

    /// Whether the agent at `index` is asked for skills behind a dollar sign.
    pub fn agent_has_skills(&self, index: usize) -> bool {
        self.agent_record(index).is_some_and(|agent| {
            pm_acp::Product::of(&format!(
                "{} {}",
                agent.command,
                agent.package.as_deref().unwrap_or_default()
            )) == Some(pm_acp::Product::Codex)
        })
    }

    /// How the agent at `index` is shown; a plain agent when there is none there.
    pub fn agent_look(&self, index: usize) -> AgentLook {
        match self.agent_record(index) {
            Some(agent) => AgentLook {
                icon: agent_icon(agent),
                name: agent.name.clone(),
            },
            None => AgentLook {
                icon: IconName::Terminal,
                name: "Agent".to_owned(),
            },
        }
    }

    /// The state with `servers` as the agents that can be started.
    pub fn with_agent_servers(mut self, servers: Vec<AgentServer>) -> Self {
        self.set_agents(servers);
        self
    }

    /// Makes `servers` the agents that can be started, carrying everything kept per agent over to the agent's new position, retiring the agents that sessions still run.
    pub(crate) fn set_agents(&mut self, servers: Vec<AgentServer>) {
        let before: Vec<AgentServer> = self
            .settings_servers
            .agents
            .iter()
            .chain(&self.settings_servers.retired)
            .cloned()
            .collect();
        let listed = self.settings_servers.agents.len();
        let used: Vec<usize> = self.sessions.iter().map(|session| session.agent).collect();
        let retired: Vec<AgentServer> = before
            .iter()
            .enumerate()
            .filter(|(index, old)| {
                used.contains(index) && !servers.iter().any(|new| new.id == old.id)
            })
            .map(|(_, old)| old.clone())
            .collect();
        let after: Vec<&AgentServer> = servers.iter().chain(&retired).collect();
        let moved: Vec<Option<usize>> = before
            .iter()
            .map(|old| after.iter().position(|new| new.id == old.id))
            .collect();
        let origin: Vec<Option<usize>> = servers
            .iter()
            .map(|new| before[..listed].iter().position(|old| old.id == new.id))
            .collect();
        carry(&mut self.agent_settings, &origin, listed);
        carry(&mut self.agent_commands, &origin, listed);
        carry(&mut self.identities, &origin, listed);
        carry(&mut self.defaults, &origin, listed);
        for (settings, commands) in self.discovered_elsewhere.values_mut() {
            carry(settings, &origin, listed);
            carry(commands, &origin, listed);
        }
        let follow = |agent: usize| moved.get(agent).copied().flatten().unwrap_or(0);
        for session in &mut self.sessions {
            session.agent = follow(session.agent);
        }
        self.agent = follow(self.agent).min(servers.len().saturating_sub(1));
        self.touched = std::mem::take(&mut self.touched)
            .into_iter()
            .filter_map(|(agent, id)| Some((moved.get(agent).copied().flatten()?, id)))
            .collect();
        self.settings_servers.agents = servers;
        self.settings_servers.retired = retired;
    }
}
