//! The agents a workspace can be started with: the ones the app offers, one for each provider of the harness.

use pm_icon::IconName;

use crate::state::{Outgoing, WorkspaceState};

/// One agent the app ships, as the windows list it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentEntry {
    /// What the app keeps the agent under.
    pub id: String,
    /// What a row calls it.
    pub name: String,
}

impl AgentEntry {
    /// The agents the app offers.
    pub fn shipped() -> Vec<Self> {
        pm_harness::agents()
            .iter()
            .map(|agent| Self {
                id: agent.id.to_owned(),
                name: agent.name.to_owned(),
            })
            .collect()
    }
}

/// How an agent is shown: its glyph and its name.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentLook {
    /// The glyph on its chip.
    pub icon: IconName,
    /// What it is called.
    pub name: String,
}

/// The glyphs of the providers that have one of their own, by provider id; every other agent is drawn with [`IconName::Terminal`].
const PROVIDER_ICONS: [(&str, IconName); 3] = [
    ("claude", IconName::Sparkles),
    ("copilot", IconName::Git),
    ("openrouter", IconName::Globe),
];

/// The glyph an agent is drawn with: the one of its provider, or a terminal when the provider has none of its own.
pub fn agent_icon(agent: &AgentEntry) -> IconName {
    pm_harness::agent(&agent.id)
        .and_then(|offered| {
            PROVIDER_ICONS
                .iter()
                .find(|(provider, _)| *provider == offered.provider)
        })
        .map_or(IconName::Terminal, |(_, icon)| *icon)
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

    /// Whether the agent at `index` is offered when a new session is started, which it is unless it was hidden in the settings.
    pub fn agent_shown(&self, index: usize) -> bool {
        self.agent_record(index)
            .is_some_and(|agent| !self.machine.agents.hidden.contains(&agent.id))
    }

    /// Whether the agent at `index` can run work: its default account is signed in or has an API key, or the reader added a login for it.
    pub fn agent_ready(&self, index: usize) -> bool {
        matches!(self.defaults.get(index), Some(Some(Some(_))))
            || self
                .agent_record(index)
                .is_some_and(|agent| self.logins.iter().any(|(owner, _, _)| *owner == agent.id))
    }

    /// The positions of the agents a new session can be started with, in the order the chips show them: the ones offered, not hidden and signed in.
    ///
    /// While no agent shown is known to be signed in, as before the sign-ins are read or on a machine with none, the agent picked is offered alone, or the first one shown when the one picked is hidden, so that work can still be started and signed in for.
    pub fn startable_agents(&self) -> Vec<usize> {
        let shown: Vec<usize> = self
            .offered_agents()
            .into_iter()
            .filter(|index| self.agent_shown(*index))
            .collect();
        let ready: Vec<usize> = shown
            .iter()
            .copied()
            .filter(|index| self.agent_ready(*index))
            .collect();
        if !ready.is_empty() {
            return ready;
        }
        let picked = shown
            .iter()
            .copied()
            .find(|index| *index == self.agent)
            .or_else(|| shown.first().copied());
        picked.into_iter().collect()
    }

    /// Hides the agent at `index` from new sessions, or shows it again, and has the server keep the choice; the last agent shown cannot be hidden.
    pub(crate) fn toggle_agent_shown(&mut self, index: usize) {
        let Some(id) = self
            .agent_id_at(index)
            .filter(|_| index < self.agent_slots())
        else {
            return;
        };
        let hidden = &mut self.machine.agents.hidden;
        if let Some(place) = hidden.iter().position(|held| *held == id) {
            hidden.remove(place);
        } else if self.shown_agents() > 1 {
            self.machine.agents.hidden.push(id);
        } else {
            return;
        }
        self.keep_startable_agent();
        if self.machine_loaded {
            self.outbox
                .push(Outgoing::SaveMachineSettings(self.machine.clone()));
        }
    }

    /// How many of the agents offered are not hidden.
    fn shown_agents(&self) -> usize {
        self.offered_agents()
            .into_iter()
            .filter(|index| self.agent_shown(*index))
            .count()
    }

    /// Moves the agent new work is given to onto the first one a new session can be started with, when the one picked was hidden or is not signed in.
    pub(crate) fn keep_startable_agent(&mut self) {
        if self.startable_agents().contains(&self.agent) {
            return;
        }
        if let Some(first) = self.startable_agents().first() {
            self.agent = *first;
        }
    }

    /// The agent at `index`: one offered, or, past those, one taken out of the list that sessions still run.
    fn agent_record(&self, index: usize) -> Option<&AgentEntry> {
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
        let record = AgentEntry {
            id: id.to_owned(),
            name: id.to_owned(),
        };
        self.settings_servers.retired.push(record);
        self.agent_slots() + self.settings_servers.retired.len() - 1
    }

    /// Whether the agent at `index` can keep more than one login, which an agent whose sign-in names an account can.
    pub fn agent_has_accounts(&self, index: usize) -> bool {
        self.agent_record(index)
            .and_then(|record| pm_harness::agent(&record.id))
            .is_some_and(|agent| agent.has_accounts())
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
    pub fn with_agent_servers(mut self, servers: Vec<AgentEntry>) -> Self {
        self.set_agents(servers);
        self
    }

    /// Makes `servers` the agents that can be started, carrying everything kept per agent over to the agent's new position, retiring the agents that sessions still run.
    pub(crate) fn set_agents(&mut self, servers: Vec<AgentEntry>) {
        let before: Vec<AgentEntry> = self
            .settings_servers
            .agents
            .iter()
            .chain(&self.settings_servers.retired)
            .cloned()
            .collect();
        let listed = self.settings_servers.agents.len();
        let used: Vec<usize> = self
            .sessions
            .iter()
            .filter(|session| !session.home)
            .map(|session| session.agent)
            .collect();
        let retired: Vec<AgentEntry> = before
            .iter()
            .enumerate()
            .filter(|(index, old)| {
                used.contains(index) && !servers.iter().any(|new| new.id == old.id)
            })
            .map(|(_, old)| old.clone())
            .collect();
        let after: Vec<&AgentEntry> = servers.iter().chain(&retired).collect();
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

#[cfg(test)]
/// Tests for hiding agents from new sessions and offering only those signed in.
mod tests {
    use super::*;

    /// A workspace whose every agent's default account is signed in.
    fn signed_in_everywhere() -> WorkspaceState {
        let mut state = WorkspaceState::new(&[]);
        for held in &mut state.defaults {
            *held = Some(Some("Signed in".to_owned()));
        }
        state
    }

    /// Only the agents signed in, by their default account, an added login or an API key, are offered for new sessions; before any is, the agent picked is offered alone, and it moves onto one signed in once one is known.
    #[test]
    fn only_signed_in_agents_start_new_sessions() {
        let mut state = WorkspaceState::new(&[]);
        let all = state.offered_agents();
        assert!(all.len() > 3);
        state.agent = all[0];
        assert_eq!(state.startable_agents(), [all[0]]);
        let key = state.agent_id_at(all[2]).unwrap();
        state
            .logins
            .push((key, "work".to_owned(), "work".to_owned()));
        state = state.with_identity(all[1], None, None, None);
        state = state.with_identity(all[3], Some("API key saved".to_owned()), None, None);
        assert_eq!(state.startable_agents(), [all[2], all[3]]);
        assert_eq!(state.agent, all[2]);
        assert!(!state.agent_ready(all[0]));
        assert!(!state.agent_ready(all[1]));
    }

    /// A hidden agent leaves the chips of a new session but stays listed, the picked agent moves off it, the choice is kept, and the last one shown stays.
    #[test]
    fn hidden_agents_leave_new_sessions_only() {
        let mut state = signed_in_everywhere();
        state.machine_loaded = true;
        let all = state.offered_agents();
        assert!(all.len() > 1);
        state.agent = all[0];
        state.toggle_agent_shown(all[0]);
        assert!(!state.startable_agents().contains(&all[0]));
        assert_eq!(state.offered_agents(), all);
        assert_ne!(state.agent, all[0]);
        assert!(state.take_outbox().iter().any(|sent| matches!(
            sent,
            Outgoing::SaveMachineSettings(settings) if settings.agents.hidden.len() == 1
        )));
        for index in &all[1..] {
            state.toggle_agent_shown(*index);
        }
        assert_eq!(state.startable_agents().len(), 1);
        state.toggle_agent_shown(all[0]);
        assert!(state.startable_agents().contains(&all[0]));
    }
}
