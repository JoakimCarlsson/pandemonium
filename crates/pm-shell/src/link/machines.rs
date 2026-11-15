//! The link's part in projects on other machines: asks the server of each
//! machine a project lives on for its sessions, shows how each server
//! stands on its projects' rows, asks again whenever a server lets the
//! window in after a lost connection, and hands every server reached the
//! machine settings the reader keeps; and follows the events of the
//! sessions they hold, under numbers of the window's own, handing every
//! request about them, and about their folders, shells and debugging, to
//! their machine's server.

use pm_client::Incoming;
use pm_protocol::{Request, Response, SessionInfo};

use super::remote::Standing;
use super::{Ask, Link, age, same_folder};
use crate::model::{ProjectReach, Session};
use crate::thread::Thread;

impl Link {
    /// The hosts of the projects on other machines, each once, in the projects' order.
    fn remote_hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = Vec::new();
        for path in &self.paths {
            if let Some((host, _)) = pm_protocol::host_of(path)
                && !hosts.iter().any(|kept| kept == host)
            {
                hosts.push(host.to_owned());
            }
        }
        hosts
    }

    /// Asks the server of every machine a project lives on for its sessions.
    pub(super) fn ask_remote_sessions(&mut self) {
        for host in self.remote_hosts() {
            self.ask_remote(&host, Request::Sessions, Ask::RemoteSessions(host.clone()));
        }
    }

    /// Sends `request` for `ask` to the server of `host`, reaching it first when it is not yet.
    pub(super) fn ask_remote(&mut self, host: &str, request: Request, ask: Ask) {
        let tokens = self.tokens();
        self.remotes.ask(host, request, ask, &tokens);
    }

    /// Hands the server of every other machine the window reaches the machine settings the reader keeps.
    pub(super) fn share_machine_settings(&mut self) {
        for host in self.remotes.hosts() {
            self.hand_machine_settings(&host);
        }
    }

    /// Hands the server of `host` the machine settings the reader keeps, once they were read from this machine.
    fn hand_machine_settings(&mut self, host: &str) {
        if !self.settings.machine_loaded {
            return;
        }
        let request = Request::SaveMachineSettings {
            settings: Box::new(self.settings.machine.clone()),
        };
        self.ask_remote(host, request, Ask::Nothing);
    }

    /// Takes in what the servers of other machines did: asks each one that let the window in again for its sessions and hands it the machine settings, and takes the sessions they list; gives back whether anything arrived.
    pub(super) fn take_remote(&mut self, answers: Vec<(String, Ask, Response)>) -> bool {
        let connected = self.remotes.take_connected();
        let changed = !connected.is_empty() || !answers.is_empty();
        for host in connected {
            self.hand_machine_settings(&host);
            if let Some(available) = self.web.announced() {
                self.ask_remote(&host, Request::BrowserAvailable { available }, Ask::Nothing);
            }
            if self.remote_hosts().contains(&host) {
                self.ask_remote(&host, Request::Sessions, Ask::RemoteSessions(host.clone()));
            }
        }
        for (from, ask, response) in answers {
            match (ask, response) {
                (Ask::RemoteSessions(host), Response::Sessions { sessions }) => {
                    self.follow_remote(&host, &sessions);
                    self.remote_sessions.insert(host, sessions);
                }
                (Ask::RemoteSessions(_), _) => {}
                (Ask::Folders(host), response) => self.answer_folders(host, response),
                (ask, response) => {
                    let response = self.routes.incoming(&from, response);
                    self.answer(ask, response);
                }
            }
        }
        changed
    }

    /// Starts following the events of each of `sessions` the server of `host` holds that the window does not follow yet.
    fn follow_remote(&mut self, host: &str, sessions: &[SessionInfo]) {
        for info in sessions.iter().filter(|info| !info.closed) {
            let id = self.routes.local(host, "session", info.session);
            if self.threads.contains_key(&id) {
                continue;
            }
            self.threads.insert(id, Thread::default());
            self.remotes.watch(host, info.session);
        }
    }

    /// Takes in `message` the server of `host` sent unasked, with what it numbers given the window's numbers; a session added or closed there has the server asked for its sessions again.
    pub(super) fn take_pushed(&mut self, host: &str, message: Incoming) {
        let message = match message {
            Incoming::Event {
                session,
                seq,
                event,
            } => Incoming::Event {
                session: self.routes.local(host, "session", session),
                seq,
                event,
            },
            Incoming::TerminalOutput { terminal, data } => Incoming::TerminalOutput {
                terminal: self.routes.local(host, "terminal", terminal),
                data,
            },
            Incoming::TerminalExited(terminal) => {
                Incoming::TerminalExited(self.routes.local(host, "terminal", terminal))
            }
            Incoming::Debug {
                debug,
                view,
                events,
            } => Incoming::Debug {
                debug: self.routes.local(host, "debug", debug),
                view,
                events,
            },
            Incoming::SessionAdded(_) | Incoming::SessionClosed(_) => {
                self.ask_remote(
                    host,
                    Request::Sessions,
                    Ask::RemoteSessions(host.to_owned()),
                );
                return;
            }
            _ => return,
        };
        self.handle(message);
    }

    /// Gives each project on another machine how its server stands and the sessions it lists, `now` being the time in milliseconds.
    pub(super) fn place_remote_projects(&mut self, now: u64) {
        for project in &mut self.workspace.projects {
            let Some((host, folder)) = pm_protocol::host_of(&project.path) else {
                continue;
            };
            project.reach = Some(match self.remotes.standing(host) {
                None | Some(Standing::Reaching) => ProjectReach::Reaching,
                Some(Standing::Reached) => ProjectReach::Reached,
                Some(Standing::Lost(reason)) => ProjectReach::Unreachable(reason),
            });
            let folder = format!("/{}", folder.trim_start_matches('/'));
            let mut listed: Vec<&SessionInfo> = self
                .remote_sessions
                .get(host)
                .into_iter()
                .flatten()
                .filter(|info| !info.closed && !info.chat && same_folder(&info.repository, &folder))
                .collect();
            listed.sort_by_key(|info| std::cmp::Reverse(info.updated));
            let routes = &mut self.routes;
            let threads = &self.threads;
            project.sessions = listed
                .into_iter()
                .map(|info| {
                    let id = routes.local(host, "session", info.session);
                    Session {
                        id,
                        title: info.title.clone(),
                        age: age(now.saturating_sub(info.updated)),
                        status: threads
                            .get(&id)
                            .map(|thread| thread.status(info.running))
                            .unwrap_or_default(),
                        branch: info.branch.clone().unwrap_or_default(),
                    }
                })
                .collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::time::{Duration, Instant};

    use pm_protocol::{ClientMessage, MachineSettings, ServerMessage, VERSION};
    use tungstenite::{Message, WebSocket};

    use super::*;

    /// Sends `message` to the client on `socket`.
    fn reply(socket: &mut WebSocket<TcpStream>, message: ServerMessage) {
        let text = serde_json::to_string(&message).expect("a message serializes");
        let _ = socket.send(Message::text(text));
    }

    /// A made-up server: lets one client in, hands `seen` every request it makes, lists `sessions`, tells each session watched that its agent finished a turn, and answers everything else as done.
    fn serve(listener: TcpListener, seen: Sender<Request>, sessions: Vec<SessionInfo>) {
        let (stream, _) = listener.accept().expect("a client connects");
        let mut socket = tungstenite::accept(stream).expect("the handshake completes");
        while let Ok(message) = socket.read() {
            let Message::Text(text) = message else {
                continue;
            };
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(ClientMessage::Hello { .. }) => reply(
                    &mut socket,
                    ServerMessage::Ready {
                        version: VERSION,
                        server: "fake".to_owned(),
                    },
                ),
                Ok(ClientMessage::Request { id, request }) => {
                    if let Request::Attach { session, .. } = &request {
                        reply(
                            &mut socket,
                            ServerMessage::Event {
                                session: *session,
                                seq: 1,
                                event: pm_harness::Event::Finished(None),
                            },
                        );
                    }
                    let response = match request {
                        Request::Sessions => Response::Sessions {
                            sessions: sessions.clone(),
                        },
                        _ => Response::Done,
                    };
                    let _ = seen.send(request);
                    reply(&mut socket, ServerMessage::Response { id, response });
                }
                Err(_) => {}
            }
        }
    }

    /// Pumps `link` until the made-up server behind `seen` was handed machine settings, and gives them back.
    fn handed(link: &mut Link, seen: &Receiver<Request>) -> MachineSettings {
        let since = Instant::now();
        while since.elapsed() < Duration::from_secs(20) {
            link.pump_remotes();
            while let Ok(request) = seen.try_recv() {
                if let Request::SaveMachineSettings { settings } = request {
                    return *settings;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("the server was never handed the machine settings");
    }

    /// The server of a project's machine is handed the machine settings once it lets the window in, and again whenever they change.
    #[test]
    fn machine_settings_reach_other_servers() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port is free");
        let address = listener.local_addr().expect("the port is known");
        let (sender, seen) = channel();
        std::thread::spawn(move || serve(listener, sender, Vec::new()));
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.settings.machine.shell.program = "fish".to_owned();
        link.settings.machine_loaded = true;
        link.set_projects(vec![format!("tcp://{address}/home/me/app")]);
        assert_eq!(handed(&mut link, &seen).shell.program, "fish");
        link.settings.machine.shell.program = "zsh".to_owned();
        link.share_machine_settings();
        assert_eq!(handed(&mut link, &seen).shell.program, "zsh");
    }

    /// A session the server of a project's machine holds is listed under the project, its finished turn is told, it opens with what that server says of it, its folder's shell, tasks and debug scenarios are asked of that server, and a prompt sent to it goes to that server under its own number.
    #[test]
    fn remote_sessions_open_and_follow() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port is free");
        let address = listener.local_addr().expect("the port is known");
        let session = SessionInfo {
            session: 7,
            agent: "claude".to_owned(),
            repository: "/home/me/app".to_owned(),
            directory: "/home/me/app".to_owned(),
            branch: Some("fix-parser".to_owned()),
            head: None,
            base: None,
            title: "Fix the parser".to_owned(),
            updated: 0,
            last_seq: 0,
            closed: false,
            running: false,
            login: None,
            submodule: None,
            chat: false,
        };
        let (sender, seen) = channel();
        std::thread::spawn(move || serve(listener, sender, vec![session]));
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.set_projects(vec![format!("tcp://{address}/home/me/app")]);
        let since = Instant::now();
        while link.finished.is_empty() && since.elapsed() < Duration::from_secs(20) {
            link.pump_remotes();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(link.finished, vec!["Fix the parser".to_owned()]);
        let listed = link.workspace.projects[0].sessions[0].clone();
        assert_eq!(listed.title, "Fix the parser");
        link.change_dock(crate::dock::Dock::connected);
        link.open_session(vec![0, 0]);
        assert_eq!(link.workspace.page, crate::page::Page::Session(listed.id));
        assert_eq!(link.workspace.title, "Fix the parser");
        assert_eq!(link.workspace.branch, "fix-parser");
        link.send_prompt("go on".to_owned());
        let since = Instant::now();
        let mut prompted = None;
        let mut asked = Vec::new();
        while prompted.is_none() && since.elapsed() < Duration::from_secs(20) {
            link.pump_remotes();
            while let Ok(request) = seen.try_recv() {
                match request {
                    Request::Prompt { session, text, .. } => prompted = Some((session, text)),
                    request => asked.push(request),
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(prompted, Some((7, "go on".to_owned())));
        let folder = "/home/me/app".to_owned();
        assert!(asked.contains(&Request::Tasks {
            directory: folder.clone()
        }));
        assert!(asked.iter().any(|request| matches!(
            request,
            Request::OpenTerminal { directory, .. } if *directory == folder
        )));
        assert!(asked.iter().any(|request| matches!(
            request,
            Request::DebugScenarios { directory, .. } if *directory == folder
        )));
    }
}
