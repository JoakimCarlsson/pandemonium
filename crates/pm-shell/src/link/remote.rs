//! The link's connections to the servers of other machines: each is reached
//! in the background the way its host says, over ssh, straight, inside WSL,
//! inside a dev container or through a tunnel, and keeps the requests made
//! before it was reached until it is.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};

use pm_client::{Client, Incoming, Wake};
use pm_protocol::{Request, Response, Route, Target};

use super::Ask;

/// How a machine's server stands with the window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Standing {
    /// It is being reached.
    #[default]
    Reaching,
    /// It answers.
    Reached,
    /// It could not be reached or the connection was lost, for this reason.
    Lost(String),
}

/// What [`Remotes::pump`] took from the servers of other machines.
pub(super) struct Pumped {
    /// Each answer, with the host it came from and what it was for.
    pub(super) answers: Vec<(String, Ask, Response)>,
    /// What each server sent unasked, with its host.
    pub(super) pushed: Vec<(String, Incoming)>,
    /// Whether anything changed.
    pub(super) changed: bool,
}

/// The connection to one machine's server.
#[derive(Default)]
struct Remote {
    /// The connection, once the server was reached.
    client: Option<Client>,
    /// The requests made before it was reached, each with what it is for.
    backlog: Vec<(Request, Ask)>,
    /// What each outstanding request is for, by id.
    asks: HashMap<u64, Ask>,
    /// How it stands.
    standing: Standing,
    /// The sessions whose events the window follows, to be watched once the server is reached.
    watching: Vec<u64>,
}

/// The connections to the servers of other machines, by host.
pub(super) struct Remotes {
    /// Each machine's connection.
    links: HashMap<String, Remote>,
    /// Where the background reaching reports each machine's target.
    reached: Receiver<(String, Result<Target, String>)>,
    /// What the background reaching reports through.
    reporter: Sender<(String, Result<Target, String>)>,
    /// What wakes the window when something arrives.
    wake: Wake,
    /// The hosts whose server let the window in since they were last taken.
    connected: Vec<String>,
}

impl Remotes {
    /// No connections yet, waking the window through `wake`.
    pub(super) fn new(wake: Wake) -> Self {
        let (reporter, reached) = channel();
        Self {
            links: HashMap::new(),
            reached,
            reporter,
            wake,
            connected: Vec::new(),
        }
    }

    /// How the server of `host` stands, when the window has tried to reach it.
    pub(super) fn standing(&self, host: &str) -> Option<Standing> {
        self.links.get(host).map(|remote| remote.standing.clone())
    }

    /// The hosts the window has tried to reach.
    pub(super) fn hosts(&self) -> Vec<String> {
        self.links.keys().cloned().collect()
    }

    /// Takes the hosts whose server let the window in since the last call, each once per connection.
    pub(super) fn take_connected(&mut self) -> Vec<String> {
        std::mem::take(&mut self.connected)
    }

    /// Sends `request` for `ask` to the server of `host`, reaching it first
    /// when it is not yet, with `tokens` giving each direct or tunnelled
    /// address its token.
    pub(super) fn ask(
        &mut self,
        host: &str,
        request: Request,
        ask: Ask,
        tokens: &HashMap<String, String>,
    ) {
        let lost = matches!(
            self.links.get(host).map(|remote| &remote.standing),
            Some(Standing::Lost(_))
        );
        if !self.links.contains_key(host) || lost {
            self.links.insert(host.to_owned(), Remote::default());
            self.reach(host.to_owned(), tokens.clone());
        }
        let Some(remote) = self.links.get_mut(host) else {
            return;
        };
        match &remote.client {
            Some(client) => {
                let id = client.request(request);
                remote.asks.insert(id, ask);
            }
            None => remote.backlog.push((request, ask)),
        }
    }

    /// Follows the events of the session numbered `session` on the server of `host`, from its first, across reconnections; nothing when the window never tried to reach it.
    pub(super) fn watch(&mut self, host: &str, session: u64) {
        let Some(remote) = self.links.get_mut(host) else {
            return;
        };
        if remote.watching.contains(&session) {
            return;
        }
        remote.watching.push(session);
        if let Some(client) = &remote.client {
            client.watch(session, 0);
        }
    }

    /// Reaches the server of `host` on a thread of its own and reports the target it found.
    fn reach(&self, host: String, tokens: HashMap<String, String>) {
        let reporter = self.reporter.clone();
        let wake = self.wake.clone();
        std::thread::spawn(move || {
            let target = target_of(&host, &tokens);
            let _ = reporter.send((host, target));
            wake();
        });
    }

    /// Takes what arrived from every machine: connects the servers reached,
    /// and gives back each answer with its host and what it was for, what
    /// each server sent unasked (session events, sessions added and closed,
    /// shell output and debugging news) with its host, and whether anything changed.
    pub(super) fn pump(&mut self) -> Pumped {
        let mut changed = false;
        while let Ok((host, target)) = self.reached.try_recv() {
            changed = true;
            let Some(remote) = self.links.get_mut(&host) else {
                continue;
            };
            match target {
                Ok(target) => {
                    let client = Client::connect(target, self.wake.clone());
                    for session in &remote.watching {
                        client.watch(*session, 0);
                    }
                    remote.client = Some(client);
                }
                Err(reason) => {
                    remote.standing = Standing::Lost(reason);
                    remote.backlog.clear();
                }
            }
        }
        let mut answers = Vec::new();
        let mut pushed = Vec::new();
        for (host, remote) in &mut self.links {
            let Some(client) = &remote.client else {
                continue;
            };
            for message in client.incoming() {
                changed = true;
                match message {
                    Incoming::Connected => {
                        remote.standing = Standing::Reached;
                        self.connected.push(host.clone());
                        for (request, ask) in remote.backlog.drain(..) {
                            let id = client.request(request);
                            remote.asks.insert(id, ask);
                        }
                    }
                    Incoming::Disconnected(reason) => remote.standing = Standing::Lost(reason),
                    Incoming::Response { id, response } => {
                        if let Some(ask) = remote.asks.remove(&id) {
                            answers.push((host.clone(), ask, response));
                        }
                    }
                    message @ (Incoming::Event { .. }
                    | Incoming::TerminalOutput { .. }
                    | Incoming::TerminalExited(_)
                    | Incoming::SessionAdded(_)
                    | Incoming::SessionClosed(_)
                    | Incoming::Debug { .. }) => pushed.push((host.clone(), message)),
                    _ => {}
                }
            }
        }
        Pumped {
            answers,
            pushed,
            changed,
        }
    }
}

/// The target of the server of `host`, found the way its route says, with
/// `tokens` giving each direct or tunnelled address its token.
///
/// # Errors
///
/// Why the server could not be reached.
fn target_of(host: &str, tokens: &HashMap<String, String>) -> Result<Target, String> {
    let token = |address: &str| tokens.get(address).cloned().unwrap_or_default();
    let progress = |_: &str| {};
    match pm_protocol::route_of(Some(host)) {
        Route::Local => pm_client::local(),
        Route::Direct(address) => Ok(Target {
            address: address.to_owned(),
            token: token(address),
        }),
        Route::Ssh(destination) => pm_ssh::remote(destination),
        Route::Wsl(distro) => pm_transport::wsl(distro, &progress),
        Route::Container(folder) => pm_transport::container(&folder, &progress),
        Route::Tunnel(hostname) => pm_transport::tunnel(hostname, &token(hostname)),
    }
}
