//! Creating, starting and attaching an environment, running only the setup that is due.

use super::container::{Container, ToolContext};
use super::journal::Journal;
use super::{Runtime, Shared, stop};
use crate::declaration::{self, Declaration, Hooks, ResolvedOrigin, Tools};
use crate::engine::{self, Inspected};
use crate::environment::{Capabilities, Environment, ExecutionContext, Intent, Provided, Request};
use crate::event::{Phase, Sink, State};
use crate::failure::{Failure, Stage};
use crate::forward::Forwarder;
use crate::identity::Identity;
use crate::lifecycle::{self, Target};
use crate::plan::{self, ENVIRONMENT, OWNER, Plan};
use crate::record::Record;
use crate::variables::Scope;
use pm_host::{Host, Location};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Creates, starts and attaches the environment of `request`.
pub(crate) fn open(
    shared: &Arc<Shared>,
    request: &Request,
    intent: Intent,
    sink: &Sink,
) -> Result<Environment, Failure> {
    let id = request.identity.id();
    let _claim = shared.claim(&id)?;
    let (state, forwards) = handles(shared, &id);
    let journal = Journal::start(shared.records.log(&id), true, sink.clone(), state);
    let result = run(shared, &journal, request, intent, forwards, sink);
    finish(&journal, result)
}

/// Attaches to the existing container of `identity` without creating it or running setup.
pub(crate) fn reconnect(
    shared: &Arc<Shared>,
    identity: &Identity,
    sink: &Sink,
) -> Result<Environment, Failure> {
    let id = identity.id();
    let _claim = shared.claim(&id)?;
    let (state, forwards) = handles(shared, &id);
    let journal = Journal::start(shared.records.log(&id), false, sink.clone(), state);
    let result = (|| {
        journal.phase(Phase::Checking);
        check_engine(shared)?;
        let record = shared.records.load(&id).ok_or_else(|| {
            Failure::new(
                Stage::Ownership,
                "This editor has no record of an environment for this checkout",
            )
        })?;
        let found = find(shared, &id)?.ok_or_else(|| {
            Failure::new(
                Stage::Ownership,
                "The environment's container no longer exists",
            )
        })?;
        if !found.running {
            start_existing(shared, &journal, &record, &found.name)?;
        }
        attach(
            shared,
            &journal,
            &Attachment {
                identity,
                source: Location::local(identity.checkout()),
                record: &record,
                forwards,
                attach_commands: false,
                sink,
            },
        )
    })();
    finish(&journal, result)
}

/// Brings back the machine named `name`, disconnected, from its record.
pub(super) fn restore(shared: &Arc<Shared>, name: &str) -> std::io::Result<Host> {
    let id = name.rsplit('-').next().unwrap_or_default();
    if let Some(runtime) = shared.runtimes.lock().unwrap().get(id) {
        return Ok(runtime.host.clone());
    }
    let record = shared
        .records
        .load(id)
        .filter(|record| record.container == name)
        .ok_or_else(|| {
            std::io::Error::other(format!(
                "No environment record for {name}; open its project again"
            ))
        })?;
    let sink: Sink = Arc::new(|_| {});
    let container = Arc::new(Container::new(
        shared,
        record.identity.clone(),
        context(&record),
        sink,
    ));
    let host = Host::launched(container.clone());
    shared.runtimes.lock().unwrap().insert(
        id.to_owned(),
        Runtime {
            host: host.clone(),
            container,
            forwarder: None,
            state: Arc::new(Mutex::new(State::Stopped)),
            forwards: Arc::default(),
        },
    );
    Ok(host)
}

/// The machine of `identity`'s environment as it exists now, restored from its record or standing in for one that is missing.
///
/// Nothing is started: a missing container surfaces as an error when something
/// tries to reach the machine, never as tools quietly running somewhere else.
pub(super) fn locate(shared: &Arc<Shared>, identity: &Identity) -> Location {
    let id = identity.id();
    let known = shared
        .runtimes
        .lock()
        .unwrap()
        .get(&id)
        .map(|runtime| (runtime.host.clone(), runtime.container.context().workspace));
    if let Some((host, workspace)) = known {
        return Location::new(host, workspace);
    }
    let sink: Sink = Arc::new(|_| {});
    let (context, state) = match shared.records.load(&id) {
        Some(record) => (context(&record), State::Stopped),
        None => (
            ToolContext {
                workspace: declaration::default_folder(identity.checkout()),
                ..ToolContext::default()
            },
            State::Absent,
        ),
    };
    let workspace = context.workspace.clone();
    let container = Arc::new(Container::new(shared, identity.clone(), context, sink));
    let host = Host::launched(container.clone());
    shared.runtimes.lock().unwrap().insert(
        id,
        Runtime {
            host: host.clone(),
            container,
            forwarder: None,
            state: Arc::new(Mutex::new(state)),
            forwards: Arc::default(),
        },
    );
    Location::new(host, workspace)
}

/// The state and listener handles an environment's operations share.
fn handles(
    shared: &Shared,
    id: &str,
) -> (Arc<Mutex<State>>, Arc<Mutex<Vec<crate::event::Forward>>>) {
    match shared.runtimes.lock().unwrap().get(id) {
        Some(runtime) => (runtime.state.clone(), runtime.forwards.clone()),
        None => (Arc::new(Mutex::new(State::Absent)), Arc::default()),
    }
}

/// Records the outcome of an operation in the log and the environment's state.
fn finish(journal: &Journal, result: Result<Environment, Failure>) -> Result<Environment, Failure> {
    match result {
        Ok(environment) => {
            journal.state(State::Ready);
            Ok(environment)
        }
        Err(failure) => {
            journal.log("error", &failure.message);
            for problem in &failure.problems.0 {
                journal.log("declaration", &problem.to_string());
            }
            journal.state(State::Failed(failure.message.clone()));
            Err(failure.with_log(journal.path().to_path_buf()))
        }
    }
}

/// Fails unless a Linux container engine answers.
fn check_engine(shared: &Shared) -> Result<(), Failure> {
    let info = shared.engine.info().map_err(|error| {
        Failure::new(
            Stage::Engine,
            format!("{} is not available: {error}", shared.engine.program()),
        )
    })?;
    if info.os != "linux" {
        return Err(Failure::new(
            Stage::Engine,
            format!(
                "{} runs {} containers; only Linux containers are supported",
                shared.engine.program(),
                info.os
            ),
        ));
    }
    Ok(())
}

/// The container this environment owns, found by its label rather than its name.
fn find(shared: &Shared, id: &str) -> Result<Option<Inspected>, Failure> {
    let names = shared
        .engine
        .containers_with(&format!("{ENVIRONMENT}={id}"))
        .map_err(|error| Failure::new(Stage::Engine, error.to_string()))?;
    for name in names {
        let found = shared
            .engine
            .inspect(&name)
            .map_err(|error| Failure::new(Stage::Engine, error.to_string()))?;
        if let Some(found) = found.filter(|found| {
            found
                .labels
                .get(OWNER)
                .is_some_and(|owner| owner == "pandemonium")
        }) {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// The whole of opening: read, check, create or reuse, set up, attach.
fn run(
    shared: &Arc<Shared>,
    journal: &Journal,
    request: &Request,
    intent: Intent,
    forwards: Arc<Mutex<Vec<crate::event::Forward>>>,
    sink: &Sink,
) -> Result<Environment, Failure> {
    let identity = &request.identity;
    let id = identity.id();
    journal.phase(Phase::Checking);
    check_engine(shared)?;
    if !request.source.host.is_local() {
        return Err(Failure::new(
            Stage::Declaration,
            "Environments are created from folders on this machine",
        ));
    }
    let source = request.source.path.clone();
    let path = request
        .declaration
        .clone()
        .or_else(|| declaration::discover(&source))
        .ok_or_else(|| {
            Failure::new(
                Stage::Declaration,
                format!(
                    "{} has no .devcontainer/devcontainer.json",
                    source.display()
                ),
            )
        })?;
    let declaration = Declaration::load(&path).map_err(Failure::declaration)?;
    let local_env = |name: &str| std::env::var(name).ok();
    let scope = Scope {
        local_env: &local_env,
        local_workspace: &source,
        container_workspace: None,
        container_env: None,
        id: &id,
    };
    let resolved = declaration.resolve(&scope).map_err(Failure::declaration)?;
    let plan = plan::plan(identity, &declaration, &resolved).map_err(Failure::declaration)?;
    let existing = find(shared, &id)?;
    if existing.is_none()
        && shared
            .engine
            .inspect(&plan.name)
            .map_err(|error| Failure::new(Stage::Engine, error.to_string()))?
            .is_some()
    {
        return Err(Failure::new(
            Stage::Ownership,
            format!(
                "A container named {} exists and is not owned by this editor",
                plan.name
            ),
        ));
    }
    if let Some(command) = &resolved.initialize {
        journal.phase(Phase::Initializing);
        lifecycle::on_desktop(&source, "initializeCommand", command, &|source, line| {
            journal.log(source, line)
        })
        .map_err(|message| Failure::new(Stage::Initialize, message))?;
    }
    if intent == Intent::Rebuild && existing.is_some() {
        journal.phase(Phase::Removing);
        stop::release(shared, &id);
        stop::remove(shared, journal, &id)?;
    }
    let existing = if intent == Intent::Rebuild {
        None
    } else {
        existing
    };
    let record = match existing {
        None => create(
            shared,
            journal,
            identity,
            &declaration,
            &resolved,
            &plan,
            &source,
        )?,
        Some(found) => reuse(
            shared,
            journal,
            identity,
            &declaration,
            &resolved,
            &found,
            &source,
        )?,
    };
    attach(
        shared,
        journal,
        &Attachment {
            identity,
            source: request.source.clone(),
            record: &record,
            forwards,
            attach_commands: true,
            sink,
        },
    )
}

/// Builds the image, creates the container, runs the creation commands and starts it.
fn create(
    shared: &Arc<Shared>,
    journal: &Journal,
    identity: &Identity,
    declaration: &Declaration,
    resolved: &declaration::Resolved,
    plan: &Plan,
    source: &Path,
) -> Result<Record, Failure> {
    let id = identity.id();
    image(shared, journal, resolved, plan)?;
    journal.phase(Phase::Creating);
    for volume in &plan.volumes {
        let mut command = shared.engine.command();
        command.args([
            "volume",
            "create",
            "--label",
            &format!("{OWNER}=pandemonium"),
            "--label",
            &format!("{ENVIRONMENT}={id}"),
            volume,
        ]);
        engine::stream(command, "volume", &mut |source, line| {
            journal.log(source, line)
        })
        .map_err(|error| Failure::new(Stage::Create, error.to_string()))?;
    }
    let mut command = shared.engine.command();
    command.arg("create").args(&plan.create);
    engine::stream(command, "create", &mut |source, line| {
        journal.log(source, line)
    })
    .map_err(|error| Failure::new(Stage::Create, error.to_string()))?;
    let found = shared
        .engine
        .inspect(&plan.name)
        .map_err(|error| Failure::new(Stage::Create, error.to_string()))?
        .ok_or_else(|| {
            Failure::new(
                Stage::Create,
                "The container disappeared right after it was created",
            )
        })?;
    let tools = tools(declaration, source, &id, &plan.workspace, &found.env)?;
    let record = record(
        identity,
        declaration,
        &plan.workspace,
        tools,
        &shared.engine,
        false,
    );
    save(shared, &id, &record)?;
    journal.phase(Phase::Starting);
    let mut start = shared.engine.command();
    start.args(["start", &plan.name]);
    engine::stream(start, "start", &mut |source, line| {
        journal.log(source, line)
    })
    .map_err(|error| Failure::new(Stage::Start, error.to_string()))?;
    setup(shared, journal, &record, &plan.name)?;
    let record = Record {
        created: true,
        ..record
    };
    save(shared, &id, &record)?;
    run_hook(
        shared,
        journal,
        &record,
        &plan.name,
        "postStartCommand",
        &record.hooks.post_start,
    )?;
    Ok(record)
}

/// Makes the image available, pulling or building it.
fn image(
    shared: &Shared,
    journal: &Journal,
    resolved: &declaration::Resolved,
    plan: &Plan,
) -> Result<(), Failure> {
    match &resolved.origin {
        ResolvedOrigin::Build(build) => {
            journal.phase(Phase::Building);
            let labels = plan
                .labels
                .iter()
                .filter(|label| !label.starts_with(plan::DECLARATION))
                .cloned()
                .collect::<Vec<_>>();
            let mut command = shared.engine.command();
            command.args(shared.engine.build_arguments(build, &plan.image, &labels));
            engine::stream(command, "build", &mut |source, line| {
                journal.log(source, line)
            })
            .map_err(|error| Failure::new(Stage::Image, error.to_string()))?;
        }
        ResolvedOrigin::Image(image) => {
            if !shared.engine.has_image(image) {
                journal.phase(Phase::Pulling);
                let mut command = shared.engine.command();
                command.args(["pull", image]);
                engine::stream(command, "pull", &mut |source, line| {
                    journal.log(source, line)
                })
                .map_err(|error| Failure::new(Stage::Image, error.to_string()))?;
            }
        }
    }
    if shared
        .engine
        .image_label(&plan.image, "devcontainer.metadata")
        .is_some()
    {
        journal.notice(
            "The image carries devcontainer.metadata, which is not merged into this environment",
        );
    }
    Ok(())
}

/// Reuses an existing container: starts it when stopped and finishes any interrupted setup.
fn reuse(
    shared: &Arc<Shared>,
    journal: &Journal,
    identity: &Identity,
    declaration: &Declaration,
    resolved: &declaration::Resolved,
    found: &Inspected,
    source: &Path,
) -> Result<Record, Failure> {
    let id = identity.id();
    let record = match shared.records.load(&id) {
        Some(record) if record.created => record,
        stored => {
            let tools = tools(
                declaration,
                source,
                &id,
                &resolved.workspace_folder,
                &found.env,
            )?;
            let created = stored.is_none();
            let record = record(
                identity,
                declaration,
                &resolved.workspace_folder,
                tools,
                &shared.engine,
                created,
            );
            save(shared, &id, &record)?;
            record
        }
    };
    if record.digest != declaration.digest {
        journal.notice("The declaration changed since this container was created; rebuild the environment to apply it");
    }
    if !found.running {
        start_existing(shared, journal, &record, &found.name)?;
    }
    if !record.created {
        setup(shared, journal, &record, &found.name)?;
        let record = Record {
            created: true,
            ..record
        };
        save(shared, &id, &record)?;
        return Ok(record);
    }
    Ok(record)
}

/// Starts a stopped container and runs what is due after a start.
fn start_existing(
    shared: &Shared,
    journal: &Journal,
    record: &Record,
    name: &str,
) -> Result<(), Failure> {
    journal.phase(Phase::Starting);
    let mut start = shared.engine.command();
    start.args(["start", name]);
    engine::stream(start, "start", &mut |source, line| {
        journal.log(source, line)
    })
    .map_err(|error| Failure::new(Stage::Start, error.to_string()))?;
    run_hook(
        shared,
        journal,
        record,
        name,
        "postStartCommand",
        &record.hooks.post_start,
    )
}

/// Runs the creation commands in the order the specification gives.
fn setup(shared: &Shared, journal: &Journal, record: &Record, name: &str) -> Result<(), Failure> {
    let hooks = &record.hooks;
    run_hook(
        shared,
        journal,
        record,
        name,
        "onCreateCommand",
        &hooks.on_create,
    )?;
    run_hook(
        shared,
        journal,
        record,
        name,
        "updateContentCommand",
        &hooks.update_content,
    )?;
    run_hook(
        shared,
        journal,
        record,
        name,
        "postCreateCommand",
        &hooks.post_create,
    )
}

/// Runs one lifecycle command inside the container, when the declaration has it.
fn run_hook(
    shared: &Shared,
    journal: &Journal,
    record: &Record,
    name: &str,
    field: &str,
    command: &Option<declaration::Command>,
) -> Result<(), Failure> {
    let Some(command) = command else {
        return Ok(());
    };
    journal.phase(Phase::SettingUp);
    let target = Target {
        engine: shared.engine.clone(),
        container: name.to_owned(),
        user: record.user.clone(),
        workdir: record.workspace.clone(),
        env: record.env.clone(),
    };
    lifecycle::in_container(&target, field, command, &|source, line| {
        journal.log(source, line)
    })
    .map_err(|message| Failure::new(Stage::Setup, message))
}

/// What attaching an environment to the editor needs.
struct Attachment<'a> {
    /// The environment.
    identity: &'a Identity,
    /// The checkout on the desktop.
    source: Location,
    /// What the environment was created with.
    record: &'a Record,
    /// Shared with the environment handle.
    forwards: Arc<Mutex<Vec<crate::event::Forward>>>,
    /// Whether this is an explicit attachment, which runs `postAttachCommand`.
    attach_commands: bool,
    /// Receives the machine's own later progress.
    sink: &'a Sink,
}

/// Connects the endpoint, opens the forwards and runs the attach commands.
fn attach(
    shared: &Arc<Shared>,
    journal: &Journal,
    attachment: &Attachment<'_>,
) -> Result<Environment, Failure> {
    let id = attachment.identity.id();
    let record = attachment.record;
    journal.phase(Phase::Connecting);
    let context = context(record);
    let (host, state) = {
        let mut runtimes = shared.runtimes.lock().unwrap();
        match runtimes.get_mut(&id) {
            Some(runtime) => {
                runtime.container.update(context, attachment.sink.clone());
                runtime.forwards = attachment.forwards.clone();
                (runtime.host.clone(), runtime.state.clone())
            }
            None => {
                let container = Arc::new(Container::new(
                    shared,
                    attachment.identity.clone(),
                    context,
                    attachment.sink.clone(),
                ));
                let host = Host::launched(container.clone());
                let state = Arc::new(Mutex::new(State::Absent));
                runtimes.insert(
                    id.clone(),
                    Runtime {
                        host: host.clone(),
                        container,
                        forwarder: None,
                        state: state.clone(),
                        forwards: attachment.forwards.clone(),
                    },
                );
                (host, state)
            }
        }
    };
    if !host.connected() {
        host.reconnect()
            .map_err(|error| Failure::new(Stage::Endpoint, error.to_string()))?;
    }
    let workspace = Location::new(host.clone(), &record.workspace);
    let present = host
        .fs()
        .metadata(&workspace)
        .is_ok_and(|metadata| metadata.is_dir());
    if !present {
        return Err(Failure::new(
            Stage::Endpoint,
            format!(
                "The workspace folder {} does not exist in the container",
                record.workspace
            ),
        ));
    }
    journal.phase(Phase::Forwarding);
    let forwarder = Forwarder::start(&host, &record.forward_ports);
    let forwards = forwarder.forwards().to_vec();
    *attachment.forwards.lock().unwrap() = forwards.clone();
    if let Some(runtime) = shared.runtimes.lock().unwrap().get_mut(&id) {
        runtime.forwarder = Some(forwarder);
    }
    journal.forwards(forwards);
    if attachment.attach_commands {
        journal.phase(Phase::Attaching);
        run_hook(
            shared,
            journal,
            record,
            &record.container,
            "postAttachCommand",
            &record.hooks.post_attach,
        )?;
    }
    Ok(Environment::provided(
        "docker",
        attachment.identity.clone(),
        Provided {
            source: attachment.source.clone(),
            workspace,
            context: ExecutionContext {
                user: record.user.clone(),
                env: record.env.clone(),
            },
            capabilities: Capabilities {
                filesystem: true,
                processes: true,
                terminals: true,
                port_forwarding: true,
                lifecycle: true,
                isolated_worktrees: true,
            },
            extensions: record.extensions.clone(),
            state,
            forwards: attachment.forwards.clone(),
            log: Some(journal.path().to_path_buf()),
        },
    ))
}

/// The tool context a record describes.
fn context(record: &Record) -> ToolContext {
    ToolContext {
        workspace: record.workspace.clone(),
        user: record.user.clone(),
        env: record.env.clone(),
        post_start: record.hooks.post_start.clone(),
    }
}

/// Substitutes the variables that apply to tools, now that the container's environment is known.
fn tools(
    declaration: &Declaration,
    source: &Path,
    id: &str,
    workspace: &str,
    container_env: &BTreeMap<String, String>,
) -> Result<Tools, Failure> {
    let local_env = |name: &str| std::env::var(name).ok();
    let scope = Scope {
        local_env: &local_env,
        local_workspace: source,
        container_workspace: Some(workspace),
        container_env: Some(container_env),
        id,
    };
    declaration
        .resolve_tools(&scope)
        .map_err(Failure::declaration)
}

/// The durable description of a container just created.
fn record(
    identity: &Identity,
    declaration: &Declaration,
    workspace: &str,
    tools: Tools,
    engine: &crate::engine::Engine,
    created: bool,
) -> Record {
    Record {
        identity: identity.clone(),
        container: identity.container_name(),
        declaration: declaration.path.clone(),
        digest: declaration.digest.clone(),
        engine: engine.program().to_owned(),
        workspace: workspace.to_owned(),
        user: tools.user,
        env: protected(tools.env, identity.checkout()),
        hooks: Hooks {
            initialize: None,
            ..tools.hooks
        },
        forward_ports: declaration.forward_ports.clone(),
        extensions: declaration.extensions.clone(),
        created,
    }
}

/// Adds the variables that protect a linked worktree's repository metadata, unless the declaration set its own.
fn protected(mut env: Vec<(String, String)>, source: &Path) -> Vec<(String, String)> {
    if env.iter().any(|(name, _)| name.starts_with("GIT_CONFIG_")) {
        return env;
    }
    env.extend(crate::workspace::linked_environment(source));
    env
}

/// Writes the record, failing the operation when the editor could not keep it.
fn save(shared: &Shared, id: &str, record: &Record) -> Result<(), Failure> {
    shared.records.save(id, record).map_err(|error| {
        Failure::new(
            Stage::Create,
            format!("The environment record could not be saved: {error}"),
        )
    })
}
