//! The work behind debugging: asking the server of a session's machine what a folder can be debugged as, starting the program there and doing things to it, and finding a program the server still debugs.

use pm_pages::{Outgoing, Screen};
use pm_protocol::{DebugInfo, Request, Response};
use pm_state::State;

use super::{Host, Pending, Work};

impl Work {
    /// The folder of the session at `session`, as the server of its machine knows it.
    fn debug_directory(model: &State<Screen>, session: usize) -> Option<(Host, String)> {
        let placement = Self::session_of(model, session)?;
        let directory = if placement.directory.is_empty() {
            placement.repository
        } else {
            placement.directory
        };
        Some((placement.host, directory))
    }

    /// Carries out what the window asked about debugging.
    pub(super) fn deliver_debug(&mut self, model: &State<Screen>, outgoing: Outgoing) {
        match outgoing {
            Outgoing::DebugScenarios { session, file } => {
                if let Some((host, directory)) = Self::debug_directory(model, session) {
                    self.ask(
                        &host,
                        Request::DebugScenarios { directory, file },
                        Pending::DebugScenarios(session),
                    );
                }
            }
            Outgoing::DebugProcesses { session } => {
                if let Some(placement) = Self::session_of(model, session) {
                    self.ask(
                        &placement.host,
                        Request::DebugProcesses,
                        Pending::DebugProcesses(session),
                    );
                }
            }
            Outgoing::DebugStart {
                session,
                file,
                scenario,
                files,
            } => {
                if let Some((host, directory)) = Self::debug_directory(model, session) {
                    let project =
                        Self::session_of(model, session).map(|placement| placement.repository);
                    self.ask(
                        &host,
                        Request::DebugStart {
                            directory,
                            file,
                            scenario,
                            files,
                            project,
                        },
                        Pending::DebugStarted(session),
                    );
                }
            }
            Outgoing::DebugAttach {
                session,
                adapter,
                pid,
                files,
            } => {
                if let Some((host, directory)) = Self::debug_directory(model, session) {
                    self.ask(
                        &host,
                        Request::DebugAttach {
                            directory,
                            adapter,
                            pid,
                            files,
                        },
                        Pending::DebugStarted(session),
                    );
                }
            }
            Outgoing::DebugDo {
                session,
                debug,
                action,
            } => {
                if let Some(placement) = Self::session_of(model, session) {
                    self.ask(
                        &placement.host,
                        Request::DebugDo { debug, action },
                        Pending::Nothing,
                    );
                }
            }
            _ => {}
        }
    }

    /// Asks the server on `host` which programs it still debugs, so that the window can pick up one it started before it was closed.
    pub(super) fn ask_debugs(&mut self, host: &Host) {
        self.ask(host, Request::Debugs, Pending::Debugs(host.clone()));
    }

    /// Does what an answer about debugging calls for.
    pub(super) fn answered_debug(
        &mut self,
        model: &State<Screen>,
        pending: Pending,
        response: Response,
    ) {
        match (pending, response) {
            (Pending::DebugScenarios(session), Response::DebugScenarios { scenarios }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_debug_scenarios(session, scenarios);
                });
            }
            (Pending::DebugScenarios(_), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| *screen = std::mem::take(screen).listing_failed(problem));
            }
            (
                Pending::DebugProcesses(session),
                Response::DebugProcesses {
                    adapters,
                    processes,
                },
            ) => {
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_debug_processes(session, adapters, processes);
                });
            }
            (Pending::DebugProcesses(_), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| *screen = std::mem::take(screen).listing_failed(problem));
            }
            (Pending::DebugStarted(session), response) => {
                let started = match response {
                    Response::Debug { debug, view } => Ok((debug, view)),
                    other => Err(Self::trouble_of(other)),
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_debug_started(session, started);
                });
            }
            (Pending::Debugs(origin), Response::Debugs { debugs }) => {
                self.adopt_debugs(model, &origin, debugs);
            }
            (Pending::DebugWatched(session), Response::Debug { debug, view }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_debug_started(session, Ok((debug, view)));
                });
            }
            (Pending::DebugRewatched(_), Response::Debug { debug, view }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_debug(debug, view, Vec::new());
                });
            }
            (Pending::DebugRewatched(debug), _) => {
                model.update(|screen| *screen = std::mem::take(screen).with_debug_lost(debug));
            }
            _ => {}
        }
    }

    /// Picks up the first program the server on `host` debugs in the folder of a session of this window, when the window shows none.
    fn adopt_debugs(&mut self, model: &State<Screen>, host: &Host, debugs: Vec<DebugInfo>) {
        if let Some((debug, session)) = model.read(Screen::debug_run) {
            let here =
                Self::session_of(model, session).is_some_and(|placement| placement.host == *host);
            if here {
                self.ask(
                    host,
                    Request::DebugWatch { debug },
                    Pending::DebugRewatched(debug),
                );
            }
            return;
        }
        let found = debugs.into_iter().find_map(|info| {
            let session = model.read(|screen| match screen {
                Screen::Workspace(state) => state.session_in(host, &info.directory),
                Screen::Onboarding(_) => None,
            })?;
            Some((session, info.debug))
        });
        if let Some((session, debug)) = found {
            self.ask(
                host,
                Request::DebugWatch { debug },
                Pending::DebugWatched(session),
            );
        }
    }
}
