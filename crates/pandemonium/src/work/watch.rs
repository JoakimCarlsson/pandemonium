//! Keeping project submodules and the shown git panel up to date through their servers’ folder watches.

use pm_pages::Screen;
use pm_protocol::Request;
use pm_state::State;

use super::{Host, Pending, Work};

impl Work {
    /// The machine and folder of the session the git panel is about, with its position, if one is shown.
    fn shown_folder(model: &State<Screen>) -> Option<(usize, Host, String)> {
        model.read(|screen| {
            let Screen::Workspace(state) = screen;
            let position = state.review_index()?;
            let session = state.sessions.get(position)?;
            let project = state.projects.get(session.project)?;
            Some((position, project.host(), session.directory.clone()))
        })
    }

    /// The projects on `host`, with their positions and folders as the server knows them.
    fn project_folders(model: &State<Screen>, host: &Host) -> Vec<(usize, String)> {
        model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state
                .projects
                .iter()
                .enumerate()
                .filter(|(_, project)| project.host() == *host)
                .map(|(index, project)| (index, project.directory().to_string_lossy().into_owned()))
                .collect()
        })
    }

    /// Watches every project root and the shown session, refreshing newly watched projects and those reached again after reconnecting.
    pub(super) fn watch_shown(&mut self, model: &State<Screen>) {
        let shown = Self::shown_folder(model).map(|(_, host, directory)| (host, directory));
        let hosts: Vec<Host> = self.links.keys().cloned().collect();
        for host in hosts {
            let projects = Self::project_folders(model, &host);
            let mut wanted: Vec<String> = projects.iter().map(|(_, path)| path.clone()).collect();
            if let Some((shown_host, directory)) = &shown
                && *shown_host == host
            {
                wanted.push(directory.clone());
            }
            wanted.sort();
            wanted.dedup();
            if self.watched.get(&host) == Some(&wanted) {
                continue;
            }
            let previous = self
                .watched
                .insert(host.clone(), wanted.clone())
                .unwrap_or_default();
            self.ask(
                &host,
                Request::WatchFolders {
                    directories: wanted,
                },
                Pending::Nothing,
            );
            for (project, directory) in projects {
                if !previous.contains(&directory) {
                    self.ask(
                        &host,
                        Request::DevContainer {
                            directory: directory.clone(),
                        },
                        Pending::DevContainer(project),
                    );
                    self.ask(
                        &host,
                        Request::Submodules { directory },
                        Pending::Submodules(project),
                    );
                }
            }
        }
    }

    /// Refreshes matching project submodules and reads the git panel again when `directory`, which the server on `host` watches, is the folder of the session shown.
    pub(super) fn folder_changed(&mut self, model: &State<Screen>, host: &Host, directory: &str) {
        for (project, root) in Self::project_folders(model, host) {
            if root == directory {
                self.ask(
                    host,
                    Request::DevContainer {
                        directory: root.clone(),
                    },
                    Pending::DevContainer(project),
                );
                self.ask(
                    host,
                    Request::Submodules { directory: root },
                    Pending::Submodules(project),
                );
            }
        }
        if let Some((position, shown_host, shown)) = Self::shown_folder(model)
            && shown_host == *host
            && shown == directory
        {
            self.refresh(model, position);
            model.update(|screen| *screen = std::mem::take(screen).with_tree_stale());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::Link;
    use pm_components::Project;
    use pm_pages::WorkspaceState;

    /// All project roots are watched on their own machines even without a session, and reconnecting refreshes them again.
    #[test]
    fn watches_projects_and_refreshes_after_reconnect() {
        let local = None;
        let remote = Some("builder".to_owned());
        let model = State::new(Screen::Workspace(WorkspaceState::new(&[
            Project::from_source("/work/one").unwrap(),
            Project::from_source("ssh://builder/work/two").unwrap(),
        ])));
        let mut work = Work::new(std::sync::Arc::new(|| {}));
        for host in [&local, &remote] {
            work.links.insert(host.clone(), Link::default());
        }
        work.watch_shown(&model);
        for (host, path, project) in [(&local, "/work/one", 0), (&remote, "/work/two", 1)] {
            let backlog = &work.links[host].backlog;
            assert!(backlog.iter().any(|(request, _)| matches!(request,
                Request::WatchFolders { directories } if directories == &[path])));
            assert!(backlog.iter().any(|(request, pending)| matches!((request, pending),
                (Request::Submodules { directory }, Pending::Submodules(index)) if directory == path && *index == project)));
        }
        let count = work.links[&remote].backlog.len();
        work.watch_shown(&model);
        assert_eq!(work.links[&remote].backlog.len(), count);
        work.watched.remove(&remote);
        work.watch_shown(&model);
        assert_eq!(work.links[&remote].backlog.len(), count + 3);
        work.folder_changed(&model, &remote, "/work/two");
        assert!(matches!(work.links[&remote].backlog.last(),
            Some((Request::Submodules { directory }, Pending::Submodules(1))) if directory == "/work/two"));
        let count = work.links[&local].backlog.len();
        work.folder_changed(&model, &local, "/work/two");
        assert_eq!(work.links[&local].backlog.len(), count);
    }
}
