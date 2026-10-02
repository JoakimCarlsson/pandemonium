//! Adding, editing and removing the tool servers every agent is opened with.
//!
//! The Agents page of the settings pane lists them; a server is described in
//! three prompts that follow one another — its name, where it is, and the
//! variables it is given — and written to the settings file at the end of
//! the third. A change applies to agents started after it.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use pm_acp::McpServer;

use crate::app::{App, Wake};
use crate::picker::Kind;
use crate::settings::Catalog;

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

/// A server being described, between one prompt and the next.
#[derive(Clone, Debug)]
pub(super) struct Draft {
    /// The name of the server being edited, when one is.
    replacing: Option<String>,
    /// The server as it was, or as the registry lists it, whose way of being
    /// reached is kept while its address is left as it was.
    original: Option<McpServer>,
    /// What the first prompt was answered with.
    name: String,
    /// What the second prompt was answered with.
    target: String,
}

impl App {
    /// Starts describing a server that is new.
    pub(super) fn add_mcp_server(&mut self) {
        self.mcp_draft = Some(Draft {
            replacing: None,
            original: None,
            name: String::new(),
            target: String::new(),
        });
        self.open_picker_with(Kind::McpName, Vec::new(), String::new());
    }

    /// Starts describing the `index`-th server again, from what it is now.
    pub(super) fn edit_mcp_server(&mut self, index: usize) {
        let Some(server) = self.mcp_servers.get(index) else {
            return;
        };
        self.mcp_draft = Some(Draft {
            replacing: Some(server.name.clone()),
            original: Some(server.clone()),
            name: server.name.clone(),
            target: server.reach.target(),
        });
        let name = server.name.clone();
        self.open_picker_with(Kind::McpName, Vec::new(), name);
    }

    /// Takes the `index`-th server away.
    pub(super) fn remove_mcp_server(&mut self, index: usize) {
        if index < self.mcp_servers.len() {
            self.mcp_servers.remove(index);
            self.offer_mcp_servers();
        }
    }

    /// Takes the name typed, and asks where the server is.
    pub(super) fn name_mcp_server(&mut self, typed: &str) {
        let Some(draft) = self.mcp_draft.as_mut() else {
            return;
        };
        if typed.trim().is_empty() {
            self.mcp_draft = None;
            self.notices.trouble("A server needs a name", None);
            return;
        }
        draft.name = typed.trim().to_owned();
        let seeded = draft.target.clone();
        self.open_picker_with(Kind::McpTarget, Vec::new(), seeded);
    }

    /// Takes the command or address typed, and asks what to give the server.
    pub(super) fn locate_mcp_server(&mut self, typed: &str) {
        let Some(draft) = self.mcp_draft.as_mut() else {
            return;
        };
        if typed.trim().is_empty() {
            self.mcp_draft = None;
            self.notices
                .trouble("A server needs a command or an address", None);
            return;
        }
        draft.target = typed.trim().to_owned();
        let seeded = draft
            .replacing
            .as_ref()
            .and_then(|name| self.mcp_servers.iter().find(|server| server.name == *name))
            .map(|server| written(server.reach.variables()))
            .unwrap_or_default();
        self.open_picker_with(Kind::McpVariables, Vec::new(), seeded);
    }

    /// Takes the variables typed, and writes the server down.
    pub(super) fn finish_mcp_server(&mut self, typed: &str) {
        let Some(draft) = self.mcp_draft.take() else {
            return;
        };
        let variables = read(typed);
        let server = match draft.original {
            Some(original) if original.reach.target() == draft.target => McpServer {
                name: draft.name.clone(),
                reach: original.reach.with_variables(variables),
            },
            _ => match McpServer::described(&draft.name, &draft.target, variables) {
                Ok(server) => server,
                Err(trouble) => return self.notices.trouble(trouble, None),
            },
        };
        let replaced = draft.replacing.as_ref().and_then(|name| {
            self.mcp_servers
                .iter()
                .position(|existing| existing.name == *name)
        });
        let named = self
            .mcp_servers
            .iter()
            .position(|existing| existing.name == server.name);
        match (replaced, named) {
            (Some(at), _) => {
                self.mcp_servers[at] = server;
                if let Some(other) = named.filter(|other| *other != at) {
                    self.mcp_servers.remove(other);
                }
            }
            (None, Some(at)) => self.mcp_servers[at] = server,
            (None, None) => self.mcp_servers.push(server),
        }
        self.offer_mcp_servers();
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
        if listing.required.is_empty() {
            self.mcp_servers.push(listing.server);
            return self.offer_mcp_servers();
        }
        self.mcp_draft = Some(Draft {
            replacing: None,
            name: listing.server.name.clone(),
            target: listing.server.reach.target(),
            original: Some(listing.server.clone()),
        });
        let seeded = written(listing.server.reach.variables());
        self.open_picker_with(Kind::McpVariables, Vec::new(), seeded);
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

/// `variables` as a reader writes them: `NAME=value`, separated by commas.
fn written(variables: &[(String, String)]) -> String {
    variables
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The variables a reader wrote, leaving out what is not `NAME=value`.
fn read(typed: &str) -> Vec<(String, String)> {
    typed
        .split(',')
        .filter_map(|entry| {
            let (name, value) = entry.split_once('=')?;
            let name = name.trim();
            (!name.is_empty()).then(|| (name.to_owned(), value.trim().to_owned()))
        })
        .collect()
}
