//! Finding the web pages that the programs of a session serve, however the program was started: by the agent, in a task or in a shell no one is watching.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use pm_protocol::ServerMessage;

use crate::core::Core;
use crate::serving::{self, Process};

/// How often the listening ports are looked at.
const LOOK: Duration = Duration::from_secs(1);

/// How many looks a new port is given to answer with a page before it is given up on.
const LOOKS: u32 = 30;

/// Whether `process` works in `folder`.
/// The processes that started it are not asked: the shell an agent starts a server from works in the agent's folder whatever the server does, so a server of another folder would count as the session's.
fn works_in(process: Process, folder: &Path) -> bool {
    serving::directory_of(process).is_some_and(|directory| directory.starts_with(folder))
}

/// The session whose folder a program listening on `port` works in, if there is one.
fn owner_of(port: u16, folders: &[(u64, PathBuf)]) -> Option<u64> {
    serving::listeners_of(port).into_iter().find_map(|process| {
        folders
            .iter()
            .find(|(_, folder)| works_in(process, folder))
            .map(|(session, _)| *session)
    })
}

impl Core {
    /// The number and folder of each session.
    fn session_folders(&self) -> Vec<(u64, PathBuf)> {
        self.sessions.lock().map_or_else(
            |_| Vec::new(),
            |sessions| {
                sessions
                    .iter()
                    .map(|(number, session)| (*number, session.directory.clone()))
                    .collect()
            },
        )
    }

    /// Looks at the listening ports once, with the ports `known` to listen and the new ones of sessions `pending` an answer with how many looks they had; tells the clients of each page found.
    fn look_for_servers(&self, known: &mut BTreeSet<u16>, pending: &mut BTreeMap<u16, (u64, u32)>) {
        let Some(listening) = serving::listening_ports() else {
            return;
        };
        known.retain(|port| listening.contains(port));
        pending.retain(|port, _| listening.contains(port));
        serving::release_unlisted(&listening);
        let folders = self.session_folders();
        let fresh: Vec<u16> = listening.difference(known).copied().collect();
        for port in &fresh {
            if let Some(session) = owner_of(*port, &folders) {
                pending.insert(*port, (session, 0));
            }
            known.insert(*port);
        }
        for (port, (session, looks)) in pending.clone() {
            let found = serving::serves_page(port);
            if found && serving::claim(port) {
                self.broadcast(
                    &ServerMessage::SessionServing {
                        session,
                        url: format!("http://localhost:{port}/"),
                    },
                    None,
                );
            }
            if found || looks + 1 >= LOOKS {
                pending.remove(&port);
            } else {
                pending.insert(port, (session, looks + 1));
            }
        }
    }
}

/// Starts looking, from now on, for the pages programs in the folders of `core`'s sessions serve; what already listens is not announced.
pub(crate) fn start(core: Arc<Core>) {
    std::thread::spawn(move || {
        let mut known = serving::listening_ports().unwrap_or_default();
        let mut pending = BTreeMap::new();
        loop {
            std::thread::sleep(LOOK);
            core.look_for_servers(&mut known, &mut pending);
        }
    });
}

#[cfg(test)]
mod tests;
