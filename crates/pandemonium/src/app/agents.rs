//! Adding, editing and removing the agents the reader runs beside the ones
//! the editor ships.
//!
//! The Agent Servers page of the settings pane lists them; an agent is
//! described in the same form an MCP server is, and written to the settings
//! file when the form is saved. An agent keeps the id its name makes, so a
//! name the editor already ships replaces that agent.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pm_acp::{Agent, Available, Install};

use crate::app::{App, Wake};
use crate::settings::{AgentCatalog, ServerForm, Subject};

/// What the agent registry last answered, shared with the threads that ask it.
#[derive(Default)]
pub(super) struct AgentRegistry {
    /// What the page shows.
    catalog: AgentCatalog,
    /// Whether the registry has been asked at all.
    asked: bool,
    /// The downloads that have finished and have not been taken in yet.
    finished: Vec<Finished>,
}

/// The agent registry, as the window and the threads asking it share it.
pub(super) type SharedAgentRegistry = Arc<Mutex<AgentRegistry>>;

/// A download that has come to an end.
struct Finished {
    /// The agent that was downloaded, as the registry lists it.
    agent: Available,
    /// The program that was unpacked, or why it was not.
    program: Result<PathBuf, String>,
}

impl App {
    /// Opens a form for an agent that is new.
    pub(super) fn add_agent_server(&mut self) {
        self.open_server_form(ServerForm::for_agent(None, None));
    }

    /// Opens the `index`-th agent the reader added out into a form.
    pub(super) fn edit_agent_server(&mut self, index: usize) {
        if let Some(agent) = self.agent_servers.get(index) {
            let form = ServerForm::for_agent(Some(index), Some(agent));
            self.open_server_form(form);
        }
    }

    /// Takes the `index`-th agent the reader added away.
    pub(super) fn remove_agent_server(&mut self, index: usize) {
        if index < self.agent_servers.len() {
            self.agent_servers.remove(index);
            self.offer_agents();
        }
    }

    /// Writes down what the form being saved describes, whichever it is.
    pub(super) fn save_server_form(&mut self) {
        match self.server_form.as_ref().map(|form| form.subject) {
            Some(Subject::McpServer) => self.save_mcp_form(),
            Some(Subject::Agent) => self.save_agent_form(),
            None => {}
        }
    }

    /// Writes the form's agent down, in place of the one it was opened on.
    fn save_agent_form(&mut self) {
        let Some(form) = self.server_form.as_ref() else {
            return;
        };
        let name = form.name.value().trim().to_owned();
        let mut words = pm_acp::command_words(&form.target.value()).into_iter();
        let id = slug(&name);
        let (Some(program), false) = (words.next(), id.is_empty()) else {
            return self
                .notices
                .trouble("An agent needs a name and a command", None);
        };
        let agent = Agent::custom(id, name, program, words.collect(), form.filled());
        let at = form.index;
        let named = self
            .agent_servers
            .iter()
            .position(|existing| existing.id == agent.id);
        match (at, named) {
            (Some(at), _) => {
                self.agent_servers[at] = agent;
                if let Some(other) = named.filter(|other| *other != at) {
                    self.agent_servers.remove(other);
                }
            }
            (None, Some(other)) => self.agent_servers[other] = agent,
            (None, None) => self.agent_servers.push(agent),
        }
        self.cancel_server_form();
        self.offer_agents();
    }

    /// What the agent registry last offered, and what is being installed from it.
    pub(super) fn agent_catalog(&self) -> AgentCatalog {
        self.agent_registry
            .lock()
            .map(|registry| registry.catalog.clone())
            .unwrap_or_default()
    }

    /// Asks the agent registry what it lists, once.
    pub(super) fn load_agent_registry(&mut self) {
        let registry = self.agent_registry.clone();
        let asked = registry
            .lock()
            .map(|mut shared| std::mem::replace(&mut shared.asked, true))
            .unwrap_or(true);
        if asked {
            return;
        }
        if let Ok(mut shared) = registry.lock() {
            shared.catalog.loading = true;
        }
        let wake = self.waker(Wake::Registry);
        std::thread::spawn(move || {
            let answer = pm_acp::fetch_agents();
            if let Ok(mut shared) = registry.lock() {
                shared.catalog.loading = false;
                match answer {
                    Ok(agents) => {
                        shared.catalog.agents = agents;
                        shared.catalog.error = None;
                    }
                    Err(error) => shared.catalog.error = Some(error),
                }
            }
            wake();
        });
    }

    /// Installs the `index`-th agent the registry offers.
    ///
    /// An agent that is run through a package runner is added at once; one
    /// that is a download is fetched away from the window, and added when
    /// it has arrived.
    pub(super) fn install_available_agent(&mut self, index: usize) {
        let Some(available) = self.agent_catalog().agents.get(index).cloned() else {
            return;
        };
        if self.agent_catalog().installing.contains(&available.id)
            || pm_acp::agents().iter().any(|agent| available.is(agent))
        {
            return;
        }
        match available.install.clone() {
            Install::Run {
                program,
                arguments,
                env,
            } => {
                self.add_agent(&available, program, arguments, env);
                self.notices
                    .done(format!("Installed {}", available.name), None);
            }
            Install::Download(download) => {
                let Some(directory) = crate::config::agents_directory() else {
                    return self
                        .notices
                        .trouble("There is nowhere to put the download", None);
                };
                let notice = self
                    .notices
                    .progress(format!("Downloading {}…", available.name));
                self.agent_downloads.push((available.id.clone(), notice));
                let registry = self.agent_registry.clone();
                if let Ok(mut shared) = registry.lock() {
                    shared.catalog.installing.push(available.id.clone());
                }
                let wake = self.waker(Wake::Registry);
                let into = directory.join(&available.id).join(&available.version);
                std::thread::spawn(move || {
                    let program = pm_acp::download_agent(&download, &into);
                    if let Ok(mut shared) = registry.lock() {
                        shared.finished.push(Finished {
                            agent: available,
                            program,
                        });
                    }
                    wake();
                });
            }
        }
    }

    /// Takes in the downloads that have finished, adding the agents that
    /// arrived and saying why the others did not.
    pub(super) fn take_agent_downloads(&mut self) {
        let finished = self
            .agent_registry
            .lock()
            .map(|mut shared| std::mem::take(&mut shared.finished))
            .unwrap_or_default();
        for Finished { agent, program } in finished {
            if let Ok(mut shared) = self.agent_registry.lock() {
                shared.catalog.installing.retain(|id| *id != agent.id);
            }
            if let Some(at) = self
                .agent_downloads
                .iter()
                .position(|(id, _)| *id == agent.id)
            {
                let (_, notice) = self.agent_downloads.remove(at);
                self.notices.dismiss(notice);
            }
            match (program, &agent.install) {
                (Ok(program), Install::Download(download)) => {
                    self.add_agent(
                        &agent,
                        program.to_string_lossy().into_owned(),
                        download.arguments.clone(),
                        download.env.clone(),
                    );
                    self.notices.done(format!("Installed {}", agent.name), None);
                }
                (Err(error), _) => self
                    .notices
                    .trouble(format!("Could not install {}: {error}", agent.name), None),
                _ => {}
            }
        }
    }

    /// Adds the registry's `available` agent, run as `program`, to the ones the reader added.
    fn add_agent(
        &mut self,
        available: &Available,
        program: String,
        arguments: Vec<String>,
        env: Vec<(String, String)>,
    ) {
        let agent = Agent::custom(
            available.id.clone(),
            available.name.clone(),
            program,
            arguments,
            env,
        );
        match self
            .agent_servers
            .iter()
            .position(|existing| existing.id == agent.id)
        {
            Some(at) => self.agent_servers[at] = agent,
            None => self.agent_servers.push(agent),
        }
        self.offer_agents();
    }

    /// Offers the agents from now on, and writes them down.
    fn offer_agents(&mut self) {
        pm_acp::install(self.agent_servers.clone());
        self.store();
    }
}

/// `name` as an id: lowercase letters and digits, joined by single dashes.
fn slug(name: &str) -> String {
    name.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
