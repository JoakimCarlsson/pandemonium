//! Adding, editing and removing the tool servers every agent is opened with.
//!
//! The Agents page of the settings pane lists them; a server is described in
//! a form that opens out under its row, every part of it in boxes at once,
//! and written to the settings file when the form is saved. A change applies
//! to agents started after it.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use pm_acp::McpServer;

use crate::app::{App, Wake, Writing};
use crate::settings::{Catalog, FormField, ServerForm};

/// How long the search box rests before the registry is asked, so that
/// typing a word is one question and not one for each letter.
const SETTLE: Duration = Duration::from_millis(300);

/// What the registry last answered, shared with the thread that asks it.
#[derive(Default)]
pub(super) struct Registry {
    /// What the page shows.
    catalog: Catalog,
    /// Which question is the latest, so that an older answer is let go.
    generation: u64,
    /// Whether the registry has been asked at all.
    asked: bool,
}

/// The registry, as the window and the thread asking it share it.
pub(super) type SharedRegistry = Arc<Mutex<Registry>>;

impl App {
    /// Opens a form for a server that is new.
    pub(super) fn add_mcp_server(&mut self) {
        self.open_server_form(ServerForm::for_mcp(None, None));
    }

    /// Opens the `index`-th server out into a form.
    pub(super) fn edit_mcp_server(&mut self, index: usize) {
        if let Some(server) = self.mcp_servers.get(index) {
            let form = ServerForm::for_mcp(Some(index), Some(server));
            self.open_server_form(form);
        }
    }

    /// Adds the `place`-th variable the registry suggests for the server
    /// being edited, with the keyboard in its value.
    pub(super) fn suggest_form_variable(&mut self, place: usize) {
        let catalog = self.mcp_catalog();
        let Some(name) = self.server_form.as_ref().and_then(|form| {
            crate::settings::suggestions(form, &catalog)
                .get(place)
                .cloned()
        }) else {
            return;
        };
        if let Some(form) = self.server_form.as_mut() {
            form.add_variable(&name);
            let at = form.variables.len() - 1;
            self.write_in(Writing::FormField(FormField::VariableValue(at)));
        }
    }

    /// Writes the form's server down, in place of the one it was opened on.
    pub(super) fn save_mcp_form(&mut self) {
        let Some(form) = self.server_form.as_ref() else {
            return;
        };
        let name = form.name.value();
        let target = form.target.value();
        let variables = form.filled();
        let server = match &form.original {
            Some(original) if original.reach.target() == target.trim() => McpServer {
                name: name.trim().to_owned(),
                reach: original.reach.clone().with_variables(variables),
                description: original.description.clone(),
                website: original.website.clone(),
                enabled: original.enabled,
            },
            original => match McpServer::described(&name, &target, variables) {
                Ok(server) => McpServer {
                    description: original
                        .as_ref()
                        .map(|original| original.description.clone())
                        .unwrap_or_default(),
                    website: original
                        .as_ref()
                        .map(|original| original.website.clone())
                        .unwrap_or_default(),
                    ..server
                },
                Err(trouble) => return self.notices.trouble(trouble, None),
            },
        };
        let at = form.index;
        let named = self
            .mcp_servers
            .iter()
            .position(|existing| existing.name == server.name);
        match (at, named) {
            (Some(at), _) => {
                self.mcp_servers[at] = server;
                if let Some(other) = named.filter(|other| *other != at) {
                    self.mcp_servers.remove(other);
                }
            }
            (None, Some(other)) => self.mcp_servers[other] = server,
            (None, None) => self.mcp_servers.push(server),
        }
        self.cancel_server_form();
        self.offer_mcp_servers();
    }

    /// Switches the `index`-th server on or off.
    pub(super) fn toggle_mcp_server(&mut self, index: usize) {
        if let Some(server) = self.mcp_servers.get_mut(index) {
            server.enabled = !server.enabled;
            self.offer_mcp_servers();
        }
    }

    /// How many running agents were given each server, in the order the servers are listed.
    pub(super) fn mcp_usage(&self) -> Vec<usize> {
        self.mcp_servers
            .iter()
            .map(|server| {
                self.agents
                    .iter()
                    .filter(|talk| talk.is_running())
                    .filter(|talk| {
                        talk.mcp_servers()
                            .iter()
                            .any(|offered| offered.given && offered.name == server.name)
                    })
                    .count()
            })
            .collect()
    }

    /// Takes the `index`-th server away.
    pub(super) fn remove_mcp_server(&mut self, index: usize) {
        if index < self.mcp_servers.len() {
            self.mcp_servers.remove(index);
            self.offer_mcp_servers();
        }
    }

    /// Installs the `index`-th server the registry offers.
    ///
    /// A server that needs nothing from the reader is added at once; one that
    /// needs a key or a token asks for the variables first, already named.
    pub(super) fn install_mcp_server(&mut self, index: usize) {
        let Some(listing) = self.mcp_catalog().listings.get(index).cloned() else {
            return;
        };
        if self
            .mcp_servers
            .iter()
            .any(|server| server.name == listing.server.name)
        {
            return;
        }
        if !listing.asks {
            self.mcp_servers.push(listing.server);
            return self.offer_mcp_servers();
        }
        let form = ServerForm::for_mcp(None, Some(&listing.server));
        self.settings.open_installed();
        self.open_server_form(form);
        self.notices.done(
            format!(
                "{} takes {}; fill in what you have and leave the rest empty",
                listing.title,
                listing.inputs.join(", ")
            ),
            None,
        );
    }

    /// Puts the configuration of the `index`-th server on the clipboard, as JSON.
    pub(super) fn copy_mcp_configuration(&mut self, index: usize) {
        if let Some(server) = self.mcp_servers.get(index) {
            crate::desktop::copy(server.configuration());
            self.notices.done("Copied the configuration", None);
        }
    }

    /// Opens the page the publisher of the `index`-th server describes it on.
    pub(super) fn open_mcp_website(&self, index: usize) {
        if let Some(server) = self.mcp_servers.get(index)
            && !server.website.is_empty()
        {
            crate::desktop::browse(&server.website);
        }
    }

    /// Shows the file the servers are written to, where the file manager keeps it.
    pub(super) fn reveal_settings_file(&self) {
        if let Some(file) = crate::config::settings_file() {
            crate::desktop::reveal(&file);
        }
    }

    /// What the registry last offered.
    pub(super) fn mcp_catalog(&self) -> Catalog {
        self.mcp_registry
            .lock()
            .map(|registry| registry.catalog.clone())
            .unwrap_or_default()
    }

    /// Asks the registry for the first page of servers, once.
    pub(super) fn load_mcp_registry(&mut self) {
        let asked = self
            .mcp_registry
            .lock()
            .map(|mut registry| std::mem::replace(&mut registry.asked, true))
            .unwrap_or(true);
        if !asked {
            self.search_mcp_registry();
        }
    }

    /// Asks the registry for what the search box holds, once the box has
    /// rested, and lets go of any answer to an earlier question.
    pub(super) fn search_mcp_registry(&mut self) {
        let query = self.mcp_search.value();
        let registry = self.mcp_registry.clone();
        let wake = self.waker(Wake::Registry);
        let generation = match registry.lock() {
            Ok(mut shared) => {
                shared.asked = true;
                shared.generation += 1;
                shared.catalog.loading = true;
                shared.generation
            }
            Err(_) => return,
        };
        std::thread::spawn(move || {
            std::thread::sleep(SETTLE);
            let current = |registry: &Registry| registry.generation == generation;
            if !registry.lock().is_ok_and(|shared| current(&shared)) {
                return;
            }
            let answer = pm_acp::search_registry(&query);
            if let Ok(mut shared) = registry.lock()
                && current(&shared)
            {
                shared.catalog.loading = false;
                match answer {
                    Ok(listings) => {
                        shared.catalog.listings = listings;
                        shared.catalog.error = None;
                    }
                    Err(error) => shared.catalog.error = Some(error),
                }
            }
            wake();
        });
    }

    /// Offers the servers to agents started from now on, and writes them down.
    fn offer_mcp_servers(&mut self) {
        pm_acp::install_mcp(self.mcp_servers.clone());
        self.store();
    }
}
