//! What the welcome page asks the server before new work starts: who each
//! agent is signed in as, so only agents that can run are offered; what the
//! agent chosen lets the reader change; and the branches a worktree can be
//! made from. And what it sends when the work starts: the settings changed,
//! the files attached and the branch picked.

use pm_protocol::Request;

use super::{Ask, Link};
use crate::composer::SlashCommand;
use crate::composer::agent_settings;

impl Link {
    /// Readies the welcome page just shown: offers the agents that can run,
    /// picks the one last used, and asks the server what it needs to know.
    pub(super) fn prepare_start(&mut self) {
        let unknown: Vec<String> = self
            .workspace
            .agents
            .iter()
            .filter(|agent| self.settings.agent_shown(&agent.id))
            .filter(|agent| !self.identities.contains_key(&agent.id))
            .map(|agent| agent.id.clone())
            .collect();
        for agent in unknown {
            self.ask(
                Request::Identity {
                    agent: agent.clone(),
                    login: None,
                },
                Ask::Identity(agent),
            );
        }
        self.offer_agents();
        let directory = self
            .workspace
            .starting
            .as_ref()
            .and_then(|start| start.at.clone())
            .and_then(|at| self.placement(&at))
            .map(|(directory, _)| directory);
        if let Some(directory) = directory {
            self.ask(
                Request::Branches {
                    directory: directory.clone(),
                },
                Ask::StartBranches(directory.clone()),
            );
            self.ask(
                Request::DevContainer {
                    directory: directory.clone(),
                },
                Ask::StartContainer(directory),
            );
        }
        self.discover_agent();
    }

    /// Lists on the welcome page the agents shown and signed in, keeping the
    /// one chosen when it is among them, else the one last used, else the first.
    fn offer_agents(&mut self) {
        let shown: Vec<usize> = self
            .workspace
            .agents
            .iter()
            .enumerate()
            .filter(|(_, agent)| self.settings.agent_shown(&agent.id))
            .map(|(index, _)| index)
            .collect();
        let ready: Vec<usize> = shown
            .iter()
            .copied()
            .filter(|index| {
                self.identities
                    .get(&self.workspace.agents[*index].id)
                    .copied()
                    .unwrap_or(false)
            })
            .collect();
        let offered = if ready.is_empty() { shown } else { ready };
        let last = self.workspace.last_agent.clone();
        let agents = &self.workspace.agents;
        let Some(start) = &mut self.workspace.starting else {
            return;
        };
        let last = offered
            .iter()
            .copied()
            .find(|index| Some(&agents[*index].id) == last.as_ref());
        if !offered.contains(&start.agent) || start.shown.is_empty() {
            start.agent = last.or(offered.first().copied()).unwrap_or(0);
        }
        start.shown = offered;
    }

    /// Asks the server what the agent chosen on the welcome page lets the reader change.
    fn discover_agent(&mut self) {
        let Some(agent) = self
            .workspace
            .starting
            .as_ref()
            .and_then(|start| self.workspace.agents.get(start.agent))
            .map(|agent| agent.id.clone())
        else {
            return;
        };
        self.ask(
            Request::Discover {
                agent: agent.clone(),
            },
            Ask::Discover(agent),
        );
    }

    /// Chooses the agent at position `index` on the welcome page, remembers it, and asks what it lets the reader change.
    pub fn choose_agent(&mut self, index: usize) {
        let Some(agent) = self
            .workspace
            .agents
            .get(index)
            .map(|agent| agent.id.clone())
        else {
            return;
        };
        let Some(start) = &mut self.workspace.starting else {
            return;
        };
        start.agent = index;
        start.settings.clear();
        start.commands.clear();
        start.changed.clear();
        self.workspace.last_agent = Some(agent);
        self.discover_agent();
    }

    /// Gives the setting `setting` of the work the welcome page starts the value `value`.
    pub(super) fn set_start_setting(&mut self, setting: String, value: String) {
        let Some(start) = &mut self.workspace.starting else {
            return;
        };
        if setting == BASE_SETTING {
            start.base = Some(value).filter(|value| !value.is_empty());
            return;
        }
        for known in &mut start.settings {
            if known.id == setting {
                known.current = value.clone();
            }
        }
        start.changed.retain(|(id, _)| *id != setting);
        start.changed.push((setting, value));
    }

    /// Handles the `response` to a request the welcome page made for `ask`.
    pub(super) fn start_answer(&mut self, ask: Ask, response: pm_protocol::Response) {
        use pm_protocol::Response;
        match (ask, response) {
            (Ask::Identity(agent), Response::Identity { default, selected }) => {
                self.identities
                    .insert(agent, default.is_some() || selected.is_some());
                self.offer_agents();
            }
            (Ask::Identity(agent), _) => {
                self.identities.insert(agent, false);
                self.offer_agents();
            }
            (Ask::Discover(agent), Response::Discovery(discovery)) => {
                let agents = &self.workspace.agents;
                if let Some(start) = &mut self.workspace.starting
                    && agents.get(start.agent).map(|known| &known.id) == Some(&agent)
                {
                    start.settings = agent_settings(&discovery.settings);
                    for (id, value) in &start.changed {
                        for known in &mut start.settings {
                            if known.id == *id {
                                known.current = value.clone();
                            }
                        }
                    }
                    start.commands = discovery
                        .commands
                        .into_iter()
                        .map(|command| SlashCommand {
                            name: command.name,
                            description: command.description,
                        })
                        .collect();
                }
            }
            (Ask::StartBranches(directory), Response::Branches { names }) => {
                let current = self
                    .workspace
                    .starting
                    .as_ref()
                    .and_then(|start| start.at.clone())
                    .and_then(|at| self.placement(&at))
                    .map(|(folder, _)| folder);
                if let Some(start) = &mut self.workspace.starting
                    && current.as_deref() == Some(directory.as_str())
                {
                    start.branches = names;
                }
            }
            (Ask::StartContainer(directory), Response::DevContainer { configured }) => {
                let current = self
                    .workspace
                    .starting
                    .as_ref()
                    .and_then(|start| start.at.clone())
                    .and_then(|at| self.placement(&at))
                    .map(|(folder, _)| folder);
                if let Some(start) = &mut self.workspace.starting
                    && current.as_deref() == Some(directory.as_str())
                {
                    start.container_offered = configured;
                }
            }
            _ => {}
        }
    }
}

/// The id of the welcome composer's choice of the branch a worktree is made from.
pub const BASE_SETTING: &str = "base";

#[cfg(test)]
/// Tests for what the welcome page offers.
mod tests {
    use std::sync::Arc;

    use pm_protocol::Response;

    use super::*;

    /// Once an agent is known to be signed in, only signed-in agents are offered, and it is picked.
    #[test]
    fn signed_in_agents_are_offered() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.new_chat();
        let first = link.workspace.agents.len();
        assert_eq!(
            link.workspace
                .starting
                .as_ref()
                .map(|start| start.shown.len()),
            Some(first)
        );
        let id = link.workspace.agents[1].id.clone();
        link.start_answer(
            Ask::Identity(id.clone()),
            Response::Identity {
                default: Some("me".to_owned()),
                selected: None,
            },
        );
        let start = link
            .workspace
            .starting
            .clone()
            .expect("the welcome page shows");
        assert_eq!(start.shown, [1]);
        assert_eq!(start.agent, 1);
    }

    /// A setting changed before starting is kept to be sent with the first prompt.
    #[test]
    fn changed_settings_are_kept() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.new_chat();
        link.set_agent_setting("mode".to_owned(), "plan".to_owned());
        link.set_agent_setting("mode".to_owned(), "auto".to_owned());
        let start = link
            .workspace
            .starting
            .clone()
            .expect("the welcome page shows");
        assert_eq!(start.changed, [("mode".to_owned(), "auto".to_owned())]);
    }
}
