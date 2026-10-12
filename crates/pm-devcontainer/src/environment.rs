//! The provider-independent environment contract.
//!
//! An environment is where one project checkout or worktree runs its tools:
//! it names the checkout, where the files are edited from, where they are
//! executed, who runs them and with what, and how far along it is. A native
//! environment is the desktop itself and needs no engine or declaration; a
//! container provider returns the same shape with a machine of its own, so
//! callers resolve tools through one seam and never learn what provided it.

use crate::event::{Forward, Sink, State};
use crate::failure::Failure;
use crate::identity::Identity;
use pm_host::{Host, Location};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// What an environment can do beyond running tools.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Capabilities {
    /// Files are read and written through the environment's machine.
    pub filesystem: bool,
    /// Processes run on the environment's machine.
    pub processes: bool,
    /// Terminals attach to the environment's machine.
    pub terminals: bool,
    /// Container ports are reachable from the desktop.
    pub port_forwarding: bool,
    /// The environment is created, stopped and rebuilt on request.
    pub lifecycle: bool,
    /// A worktree gets an environment of its own, set up before its agent starts.
    pub isolated_worktrees: bool,
}

/// Who runs tools in an environment, and with what.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExecutionContext {
    /// The user tools run as; none means the machine's own default.
    pub user: Option<String>,
    /// Variables added to every tool.
    pub env: Vec<(String, String)>,
}

/// One checkout's or worktree's environment.
#[derive(Clone, Debug)]
pub struct Environment {
    /// The provider that owns it, such as `native` or `docker`.
    provider: &'static str,
    /// The project and checkout it belongs to.
    identity: Identity,
    /// The folder on the desktop the files live in.
    source: Location,
    /// The same files as tools see them.
    workspace: Location,
    /// Who runs tools and with what.
    context: ExecutionContext,
    /// What the provider can do.
    capabilities: Capabilities,
    /// The editor extensions the declaration asks for.
    extensions: Vec<String>,
    /// Where it is in its life.
    state: Arc<Mutex<State>>,
    /// The desktop listeners currently open.
    forwards: Arc<Mutex<Vec<Forward>>>,
    /// The retained log of the latest attempt, when the provider keeps one.
    log: Option<PathBuf>,
}

impl Environment {
    /// The desktop itself, which needs no engine and no declaration.
    pub fn native(identity: Identity, source: Location) -> Self {
        Self {
            provider: "native",
            identity,
            workspace: source.clone(),
            source,
            context: ExecutionContext::default(),
            capabilities: Capabilities {
                filesystem: true,
                processes: true,
                terminals: true,
                ..Capabilities::default()
            },
            extensions: Vec::new(),
            state: Arc::new(Mutex::new(State::Ready)),
            forwards: Arc::default(),
            log: None,
        }
    }

    /// An environment a provider created.
    pub fn provided(provider: &'static str, identity: Identity, parts: Provided) -> Self {
        Self {
            provider,
            identity,
            source: parts.source,
            workspace: parts.workspace,
            context: parts.context,
            capabilities: parts.capabilities,
            extensions: parts.extensions,
            state: parts.state,
            forwards: parts.forwards,
            log: parts.log,
        }
    }

    /// The provider that owns it.
    pub fn provider(&self) -> &'static str {
        self.provider
    }

    /// The project and checkout it belongs to.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// The folder on the desktop the files live in.
    pub fn source(&self) -> &Location {
        &self.source
    }

    /// The files as tools see them; its host runs every tool.
    pub fn workspace(&self) -> &Location {
        &self.workspace
    }

    /// The machine tools run on.
    pub fn host(&self) -> &Host {
        &self.workspace.host
    }

    /// Who runs tools and with what.
    pub fn context(&self) -> &ExecutionContext {
        &self.context
    }

    /// What the provider can do.
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    /// The editor extensions the declaration asks for.
    pub fn extensions(&self) -> &[String] {
        &self.extensions
    }

    /// Where it is in its life.
    pub fn state(&self) -> State {
        self.state.lock().unwrap().clone()
    }

    /// The desktop listeners currently open.
    pub fn forwards(&self) -> Vec<Forward> {
        self.forwards.lock().unwrap().clone()
    }

    /// The retained log of the latest attempt.
    pub fn log(&self) -> Option<&std::path::Path> {
        self.log.as_deref()
    }
}

/// What a provider fills in when it creates an environment.
pub struct Provided {
    /// The folder on the desktop the files live in.
    pub source: Location,
    /// The same files as tools see them.
    pub workspace: Location,
    /// Who runs tools and with what.
    pub context: ExecutionContext,
    /// What the provider can do.
    pub capabilities: Capabilities,
    /// The editor extensions the declaration asks for.
    pub extensions: Vec<String>,
    /// Shared with the provider, which updates it as the environment changes.
    pub state: Arc<Mutex<State>>,
    /// Shared with the provider, which updates it as listeners open and close.
    pub forwards: Arc<Mutex<Vec<Forward>>>,
    /// The retained log of the latest attempt.
    pub log: Option<PathBuf>,
}

/// What opening an environment needs.
#[derive(Clone, Debug)]
pub struct Request {
    /// The project and checkout the environment belongs to.
    pub identity: Identity,
    /// The checkout or worktree folder on the desktop.
    pub source: Location,
    /// The declaration to use; found in the checkout when absent.
    pub declaration: Option<PathBuf>,
}

/// How an open request relates to an environment that may already exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intent {
    /// Create, start and attach as needed, running only the commands that are due.
    Open,
    /// Replace what the environment owns with a fresh one.
    Rebuild,
}

/// A source of environments. A container engine is the first; others follow this shape.
pub trait Provider: Send + Sync {
    /// A stable name, such as `docker`.
    fn id(&self) -> &'static str;

    /// What environments of this provider can do.
    fn capabilities(&self) -> Capabilities;

    /// The declaration of the checkout at `source`, when it has one for this provider.
    fn declaration(&self, source: &Location) -> Option<PathBuf>;

    /// Creates, starts and attaches the environment, reporting progress to `sink`.
    fn open(&self, request: &Request, intent: Intent, sink: &Sink) -> Result<Environment, Failure>;

    /// Attaches to an environment that already exists, without creating or running setup.
    fn reconnect(&self, identity: &Identity, sink: &Sink) -> Result<Environment, Failure>;

    /// Releases the environment's processes and forwards, keeping its files.
    fn stop(&self, identity: &Identity, sink: &Sink) -> Result<(), Failure>;

    /// Where the environment is now.
    fn state(&self, identity: &Identity) -> State;
}
