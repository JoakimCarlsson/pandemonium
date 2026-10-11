//! The container as a machine: how its endpoint is launched and kept running.

use super::{Shared, journal::Journal};
use crate::declaration::Command;
use crate::event::{Event, Sink, State};
use crate::identity::Identity;
use crate::lifecycle::{self, Target};
use pm_host::{Endpoint, Location};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::{Arc, Mutex, Weak};

/// What applies to tools, replaced when the environment is rebuilt without changing its machine.
#[derive(Clone, Debug, Default)]
pub(super) struct ToolContext {
    /// Where the source appears in the container.
    pub workspace: String,
    /// The user tools run as; none leaves the container's own.
    pub user: Option<String>,
    /// The variables tools run with.
    pub env: Vec<(String, String)>,
    /// What runs each time the container starts.
    pub post_start: Option<Command>,
}

/// One owned container, as the machine tools run on.
///
/// The handle is stable across stops and rebuilds: only its tool context and
/// the container behind its name change, so every location that holds the
/// machine keeps working after a reconnect.
pub(crate) struct Container {
    /// The provider's shared state, weak so a machine never keeps the provider alive.
    shared: Weak<Shared>,
    /// The container name, which is also the machine's name.
    name: String,
    /// The project and checkout that own it.
    identity: Identity,
    /// The checkout folder on the desktop.
    source: PathBuf,
    /// Receives progress for work the machine starts itself, such as restarting.
    sink: Mutex<Sink>,
    /// The tool context.
    context: Mutex<ToolContext>,
}

impl Container {
    /// A container of `identity` with the given tool context.
    pub(super) fn new(
        shared: &Arc<Shared>,
        identity: Identity,
        context: ToolContext,
        sink: Sink,
    ) -> Self {
        Self {
            shared: Arc::downgrade(shared),
            name: identity.container_name(),
            source: identity.checkout().to_path_buf(),
            identity,
            sink: Mutex::new(sink),
            context: Mutex::new(context),
        }
    }

    /// Replaces the tool context after a rebuild or a changed declaration.
    pub(super) fn update(&self, context: ToolContext, sink: Sink) {
        *self.context.lock().unwrap() = context;
        *self.sink.lock().unwrap() = sink;
    }

    /// The sink progress of the machine's own work goes to.
    fn sink(&self) -> Sink {
        self.sink.lock().unwrap().clone()
    }

    /// The project and checkout that own the container.
    pub(super) fn identity(&self) -> &Identity {
        &self.identity
    }

    /// The tool context now in force.
    pub(super) fn context(&self) -> ToolContext {
        self.context.lock().unwrap().clone()
    }

    /// Where commands run inside the container.
    pub(super) fn target(&self, shared: &Shared) -> Target {
        let context = self.context();
        Target {
            engine: shared.engine.clone(),
            container: self.name.clone(),
            user: context.user,
            workdir: context.workspace,
            env: context.env,
        }
    }
}

impl Endpoint for Container {
    /// The container name.
    fn name(&self) -> String {
        self.name.clone()
    }

    /// Stored locations of a container are prefixed `devcontainer://`.
    fn scheme(&self) -> &'static str {
        "devcontainer"
    }

    /// A `docker exec` of the script as the tool user, in the workspace, with the tool environment.
    fn shell(&self, script: &str) -> io::Result<Process> {
        let shared = self
            .shared
            .upgrade()
            .ok_or_else(|| io::Error::other("The container provider has shut down"))?;
        let context = self.context();
        let mut process = shared.engine.exec(
            &self.name,
            context.user.as_deref(),
            &context.workspace,
            &context.env,
            true,
        );
        process.args(["sh", "-c", script]);
        Ok(process)
    }

    /// Starts the container when it is stopped, running the commands due after a start.
    fn prepare(&self) -> io::Result<()> {
        let shared = self
            .shared
            .upgrade()
            .ok_or_else(|| io::Error::other("The container provider has shut down"))?;
        let found = shared.engine.inspect(&self.name)?.ok_or_else(|| {
            io::Error::other(format!(
                "The container {} no longer exists; rebuild the environment",
                self.name
            ))
        })?;
        if found.running {
            return Ok(());
        }
        let journal = Journal::start(
            shared.records.log(&self.identity.id()),
            false,
            self.sink(),
            Arc::default(),
        );
        journal.state(State::Working(crate::event::Phase::Starting));
        let mut start = shared.engine.command();
        start.args(["start", &self.name]);
        crate::engine::stream(start, "start", &mut |source, line| {
            journal.log(source, line)
        })?;
        if let Some(command) = self.context().post_start {
            lifecycle::in_container(
                &self.target(&shared),
                "postStartCommand",
                &command,
                &|source, line| journal.log(source, line),
            )
            .map_err(io::Error::other)?;
        }
        (self.sink())(Event::State(State::Ready));
        Ok(())
    }

    /// The checkout folder a path in the workspace is a bind mount of.
    fn native(&self, path: &Path) -> Option<PathBuf> {
        let workspace = self.context().workspace;
        let relative = path.strip_prefix(&workspace).ok()?;
        Some(self.source.join(relative))
    }

    /// Shown beside the project name, since the container's own name is an identifier.
    fn label(&self) -> String {
        "container".to_owned()
    }

    /// The machine of the worktree's environment as it exists now, without starting anything.
    fn locate(&self, native: &Path) -> io::Result<Location> {
        let shared = self
            .shared
            .upgrade()
            .ok_or_else(|| io::Error::other("The container provider has shut down"))?;
        Ok(super::open::locate(
            &shared,
            &Identity::new(self.identity.project(), native),
        ))
    }

    /// Removes this container and what it owns, keeping the worktree's files.
    fn retire(&self, native: &Path) -> io::Result<()> {
        let shared = self
            .shared
            .upgrade()
            .ok_or_else(|| io::Error::other("The container provider has shut down"))?;
        if native != self.source {
            return Err(io::Error::other(
                "The environment belongs to a different folder",
            ));
        }
        super::stop::retire(&shared, &self.identity, &self.sink())
            .map_err(|failure| io::Error::other(failure.to_string()))
    }

    /// Opens a worktree of this project in an environment of its own, setting it up before returning.
    fn isolate(&self, native: &Path) -> io::Result<Location> {
        let shared = self
            .shared
            .upgrade()
            .ok_or_else(|| io::Error::other("The container provider has shut down"))?;
        let identity = Identity::new(self.identity.project(), native);
        let request = crate::environment::Request {
            identity,
            source: Location::local(native),
            declaration: None,
        };
        super::open(
            &shared,
            &request,
            crate::environment::Intent::Open,
            &self.sink(),
        )
        .map(|environment| environment.workspace().clone())
        .map_err(|failure| io::Error::other(failure.to_string()))
    }
}
