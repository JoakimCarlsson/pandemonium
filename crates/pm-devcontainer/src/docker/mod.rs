//! The local Docker engine as an environment provider.

mod container;
mod journal;
mod open;
mod stop;

pub(crate) use container::Container;

use crate::engine::Engine;
use crate::environment::{Capabilities, Environment, Intent, Provider, Request};
use crate::event::{Forward, Sink, State};
use crate::failure::{Failure, Stage};
use crate::forward::Forwarder;
use crate::identity::Identity;
use crate::record::{Record, Records};
use pm_host::{Host, Location};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) use open::open;

/// The provider's preferences, read from the editor's configuration.
#[derive(Clone, Debug)]
pub struct Settings {
    /// The program that speaks the Docker command line.
    pub engine: String,
    /// Where records and logs of this editor's environments are kept.
    pub state: PathBuf,
}

/// What the provider holds for one environment while the editor runs.
pub(crate) struct Runtime {
    /// The environment's machine, stable across stops and rebuilds.
    pub host: Host,
    /// The machine's launcher, whose tool context a rebuild replaces.
    pub container: Arc<Container>,
    /// The desktop listeners, released when the environment stops.
    pub forwarder: Option<Forwarder>,
    /// Shared with the environment handle.
    pub state: Arc<Mutex<State>>,
    /// Shared with the environment handle.
    pub forwards: Arc<Mutex<Vec<Forward>>>,
}

/// State shared by the provider and the machines it launched.
pub(crate) struct Shared {
    /// The engine.
    pub engine: Engine,
    /// Records and logs.
    pub records: Records,
    /// The environments known to this editor, by identity digits.
    pub runtimes: Mutex<HashMap<String, Runtime>>,
    /// The environments an operation is working on, by identity digits.
    busy: Mutex<HashSet<String>>,
}

/// Holds one environment for the duration of an operation.
pub(crate) struct Claim {
    /// The provider's state.
    shared: Arc<Shared>,
    /// The environment held.
    id: String,
}

impl Drop for Claim {
    /// Frees the environment for the next operation.
    fn drop(&mut self) {
        self.shared.busy.lock().unwrap().remove(&self.id);
    }
}

impl Shared {
    /// Reserves the environment `id`, or says that another operation holds it.
    pub(crate) fn claim(self: &Arc<Self>, id: &str) -> Result<Claim, Failure> {
        if !self.busy.lock().unwrap().insert(id.to_owned()) {
            return Err(Failure::new(
                Stage::Busy,
                "Another operation is already working on this environment",
            ));
        }
        Ok(Claim {
            shared: self.clone(),
            id: id.to_owned(),
        })
    }
}

/// Dev containers on a local Docker engine.
#[derive(Clone)]
pub struct Docker {
    /// The state shared with launched machines.
    shared: Arc<Shared>,
}

impl Docker {
    /// A provider using the engine and state directory of `settings`.
    pub fn new(settings: Settings) -> Self {
        Self {
            shared: Arc::new(Shared {
                engine: Engine::new(settings.engine),
                records: Records::new(settings.state),
                runtimes: Mutex::new(HashMap::new()),
                busy: Mutex::new(HashSet::new()),
            }),
        }
    }

    /// Brings back the machine a stored `devcontainer://` location names, disconnected until reconnected.
    pub fn resolver(&self) -> pm_host::Resolver {
        let shared = self.shared.clone();
        Arc::new(move |scheme, name| {
            if scheme != "devcontainer" {
                return Err(std::io::Error::other(format!(
                    "Docker does not provide {scheme} locations"
                )));
            }
            open::restore(&shared, name)
        })
    }

    /// The records of every environment this editor has created.
    pub fn records(&self) -> Vec<Record> {
        self.shared.records.all()
    }

    /// The log of the latest attempt on `identity`.
    pub fn log(&self, identity: &Identity) -> PathBuf {
        self.shared.records.log(&identity.id())
    }

    /// The environment a machine belongs to, when this editor created or restored it.
    pub fn identity_of(&self, host: &Host) -> Option<Identity> {
        let name = host
            .name()
            .filter(|_| host.scheme() == Some("devcontainer"))?;
        let id = name.rsplit('-').next()?;
        if let Some(runtime) = self.shared.runtimes.lock().unwrap().get(id) {
            return Some(runtime.container.identity().clone());
        }
        self.shared.records.load(id).map(|record| record.identity)
    }

    /// The machine of `identity`, once this editor has opened or restored it.
    pub fn host(&self, identity: &Identity) -> Option<Host> {
        self.shared
            .runtimes
            .lock()
            .unwrap()
            .get(&identity.id())
            .map(|runtime| runtime.host.clone())
    }

    /// Whether the engine answers.
    pub fn available(&self) -> Result<crate::engine::Info, Failure> {
        self.shared.engine.info().map_err(|error| {
            Failure::new(
                Stage::Engine,
                format!("{} is not available: {error}", self.shared.engine.program()),
            )
        })
    }
}

impl Provider for Docker {
    /// The provider name.
    fn id(&self) -> &'static str {
        "docker"
    }

    /// Everything a container environment offers.
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            filesystem: true,
            processes: true,
            terminals: true,
            port_forwarding: true,
            lifecycle: true,
            isolated_worktrees: true,
        }
    }

    /// The declaration of a local checkout, found by convention.
    fn declaration(&self, source: &Location) -> Option<PathBuf> {
        source
            .host
            .is_local()
            .then(|| crate::declaration::discover(&source.path))
            .flatten()
    }

    /// Creates, starts and attaches the environment.
    fn open(&self, request: &Request, intent: Intent, sink: &Sink) -> Result<Environment, Failure> {
        open::open(&self.shared, request, intent, sink)
    }

    /// Attaches to the existing container without running setup.
    fn reconnect(&self, identity: &Identity, sink: &Sink) -> Result<Environment, Failure> {
        open::reconnect(&self.shared, identity, sink)
    }

    /// Releases forwards and processes, then stops the container.
    fn stop(&self, identity: &Identity, sink: &Sink) -> Result<(), Failure> {
        stop::stop(&self.shared, identity, sink)
    }

    /// Absent, stopped, working, ready or failed.
    fn state(&self, identity: &Identity) -> State {
        stop::state(&self.shared, identity)
    }
}
