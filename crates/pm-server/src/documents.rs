//! The files clients have open for their language servers, and who is told what the servers find wrong with each.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pm_lsp::{Hub, Published};
use pm_protocol::ServerMessage;

use crate::core::Connection;

/// The clients that have one file open: the folder as they named it, and their connections.
type Openers = (String, Vec<Arc<Connection>>);

/// The open files and the hub that runs their language servers.
pub(crate) struct Documents {
    /// The hub.
    pub(crate) hub: Arc<Hub>,
    /// Who opened each file, by project and path.
    openers: Arc<Mutex<HashMap<(PathBuf, String), Openers>>>,
}

impl Documents {
    /// No files open; what servers find wrong with a file goes to the connections that opened it.
    pub(crate) fn new() -> Self {
        let openers: Arc<Mutex<HashMap<(PathBuf, String), Openers>>> = Arc::default();
        let reached = openers.clone();
        let hub = Hub::new(Arc::new(move |published: Published| {
            let Some((directory, connections)) = reached.lock().ok().and_then(|held| {
                held.get(&(published.root.clone(), published.path.clone()))
                    .cloned()
            }) else {
                return;
            };
            let message = ServerMessage::Diagnostics {
                directory,
                path: published.path,
                version: published.version,
                diagnostics: published.diagnostics,
            };
            for connection in connections {
                let _ = connection.outbox.send(message.clone());
            }
        }));
        let documents = Self { hub, openers };
        documents.settings_changed(&crate::machine::settings());
        documents
    }

    /// Has each language read by the language server `settings` choose for it.
    pub(crate) fn settings_changed(&self, settings: &pm_protocol::MachineSettings) {
        let choices = settings
            .languages
            .iter()
            .filter_map(|(language, preferences)| {
                let command = preferences.server_command.trim();
                let choice = if preferences.server_off {
                    pm_lsp::ServerChoice::Off
                } else if !command.is_empty() {
                    pm_lsp::ServerChoice::Command(command.to_owned())
                } else {
                    return None;
                };
                Some((language.clone(), choice))
            })
            .collect();
        let mut choices: HashMap<String, pm_lsp::ServerChoice> = choices;
        choices.extend(crate::extensions::refused_servers());
        self.hub.choose_servers(choices);
    }

    /// Notes that `connection` has the file at `path` of the project at `root`, which it calls `directory`, open.
    pub(crate) fn opened(
        &self,
        connection: &Arc<Connection>,
        root: PathBuf,
        directory: &str,
        path: &str,
    ) {
        if let Ok(mut openers) = self.openers.lock() {
            let entry = openers
                .entry((root, path.to_owned()))
                .or_insert_with(|| (directory.to_owned(), Vec::new()));
            entry.0 = directory.to_owned();
            if !entry.1.iter().any(|held| Arc::ptr_eq(held, connection)) {
                entry.1.push(connection.clone());
            }
        }
    }

    /// Notes that `connection` closed the file at `path` of the project at `root`, and closes it for its server once no client has it open.
    pub(crate) fn closed(&self, connection: &Arc<Connection>, root: PathBuf, path: &str) {
        let key = (root, path.to_owned());
        let last = self.openers.lock().is_ok_and(|mut openers| {
            let Some(entry) = openers.get_mut(&key) else {
                return true;
            };
            entry.1.retain(|held| !Arc::ptr_eq(held, connection));
            let empty = entry.1.is_empty();
            if empty {
                openers.remove(&key);
            }
            empty
        });
        if last {
            self.hub.close(&key.0, &key.1);
        }
    }

    /// Closes every file `connection` had open, as it went away.
    pub(crate) fn forget(&self, connection: &Arc<Connection>) {
        let held: Vec<(PathBuf, String)> = self
            .openers
            .lock()
            .map(|openers| {
                openers
                    .iter()
                    .filter(|(_, (_, connections))| {
                        connections.iter().any(|held| Arc::ptr_eq(held, connection))
                    })
                    .map(|(key, _)| key.clone())
                    .collect()
            })
            .unwrap_or_default();
        for (root, path) in held {
            self.closed(connection, root, &path);
        }
    }
}
