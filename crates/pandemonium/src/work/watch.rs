//! Keeping the git panel up to date by itself: the server of the session shown watches its folder, and the panel is read again whenever the server says the folder changed.

use pm_pages::Screen;
use pm_protocol::Request;
use pm_state::State;

use super::{Host, Pending, Work};

impl Work {
    /// The machine and folder of the session the git panel is about, with its position, if one is shown.
    fn shown_folder(model: &State<Screen>) -> Option<(usize, Host, String)> {
        model.read(|screen| {
            let Screen::Workspace(state) = screen else {
                return None;
            };
            let position = state.review_index()?;
            let session = state.sessions.get(position)?;
            let project = state.projects.get(session.project)?;
            Some((position, project.host(), session.directory.clone()))
        })
    }

    /// Has the server of the session shown watch its folder, and every other server the window reaches watch nothing, telling only those whose folder changed since they were last asked.
    pub(super) fn watch_shown(&mut self, model: &State<Screen>) {
        let shown = Self::shown_folder(model).map(|(_, host, directory)| (host, directory));
        let hosts: Vec<Host> = self.links.keys().cloned().collect();
        for host in hosts {
            let wanted = shown
                .as_ref()
                .filter(|(held, _)| *held == host)
                .map(|(_, directory)| directory.clone());
            if self.watched.get(&host) == Some(&wanted) {
                continue;
            }
            self.watched.insert(host.clone(), wanted.clone());
            self.ask(
                &host,
                Request::WatchFolders {
                    directories: wanted.into_iter().collect(),
                },
                Pending::Nothing,
            );
        }
    }

    /// Reads the git panel again when `directory`, which the server on `host` watches, is the folder of the session shown.
    pub(super) fn folder_changed(&mut self, model: &State<Screen>, host: &Host, directory: &str) {
        if let Some((position, shown_host, shown)) = Self::shown_folder(model)
            && shown_host == *host
            && shown == directory
        {
            self.refresh(model, position);
        }
    }
}
