//! The work behind tasks: asking the server of a session's machine what its folder can be built, tested and run with, running a task or a runnable place of a file there in a shell shown in the dock, and choosing the toolchains a project is run with.

use pm_pages::{Outgoing, Screen};
use pm_protocol::{Request, Response};
use pm_state::State;

use super::{Host, Pending, Work};

/// Where the session at a position works, on which machine.
struct Place {
    /// The machine.
    host: Host,
    /// The folder of the session, as the server of its machine knows it.
    directory: String,
    /// The project the session belongs to, as the server of its machine knows it.
    project: String,
}

impl Work {
    /// Where the session at `session` works, and the project it belongs to.
    fn task_place(model: &State<Screen>, session: usize) -> Option<Place> {
        let placement = Self::session_of(model, session)?;
        let directory = if placement.directory.is_empty() {
            placement.repository.clone()
        } else {
            placement.directory
        };
        Some(Place {
            host: placement.host,
            directory,
            project: placement.repository,
        })
    }

    /// Carries out what the window asked about tasks.
    pub(super) fn deliver_tasks(&mut self, model: &State<Screen>, outgoing: Outgoing) {
        match outgoing {
            Outgoing::Tasks { session } => {
                if let Some(place) = Self::task_place(model, session) {
                    self.ask(
                        &place.host,
                        Request::Tasks {
                            directory: place.directory,
                        },
                        Pending::Tasks(session),
                    );
                }
            }
            Outgoing::RunTask {
                session,
                task,
                tab,
                columns,
                rows,
            } => {
                if let Some(place) = Self::task_place(model, session) {
                    self.held.insert(tab, Vec::new());
                    self.ask(
                        &place.host,
                        Request::RunTask {
                            directory: place.directory,
                            project: Some(place.project),
                            task: Box::new(task),
                            columns,
                            rows,
                        },
                        Pending::OpenTerminal(tab),
                    );
                }
            }
            Outgoing::RunRunnable {
                session,
                path,
                kind,
                name,
                tab,
                columns,
                rows,
            } => {
                if let Some(place) = Self::task_place(model, session) {
                    self.held.insert(tab, Vec::new());
                    self.ask(
                        &place.host,
                        Request::RunRunnable {
                            directory: place.directory,
                            project: Some(place.project),
                            path,
                            kind,
                            name,
                            columns,
                            rows,
                        },
                        Pending::OpenTerminal(tab),
                    );
                }
            }
            Outgoing::Toolchains { session } => {
                if let Some(place) = Self::task_place(model, session) {
                    self.ask(
                        &place.host,
                        Request::Toolchains {
                            directory: place.directory,
                            project: Some(place.project),
                        },
                        Pending::Toolchains(session),
                    );
                }
            }
            Outgoing::SelectToolchain {
                session,
                language,
                toolchain,
            } => {
                if let Some(place) = Self::task_place(model, session) {
                    self.ask(
                        &place.host,
                        Request::SelectToolchain {
                            project: place.project,
                            language,
                            toolchain,
                        },
                        Pending::Nothing,
                    );
                }
            }
            _ => {}
        }
    }

    /// Does what an answer about tasks calls for.
    pub(super) fn answered_tasks(
        &mut self,
        model: &State<Screen>,
        pending: Pending,
        response: Response,
    ) {
        match (pending, response) {
            (Pending::Tasks(session), Response::Tasks { tasks, problem }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_tasks(session, tasks, problem);
                });
            }
            (Pending::Toolchains(session), Response::Toolchains { toolchains }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_toolchains(session, toolchains);
                });
            }
            (Pending::Toolchains(_), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| *screen = std::mem::take(screen).listing_failed(problem));
            }
            _ => {}
        }
    }
}
