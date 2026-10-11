//! Dev container environments: explicit open, rebuild, reconnect and stop.

use crate::app::{App, Wake};
use crate::config;
use crate::message::Message;
use crate::notice::NoticeId;
use pm_core::Scope;
use pm_devcontainer::{
    Docker, Environment, Event, Failure, Identity, Intent, Phase, Provider, Request, Settings,
    State,
};
use pm_host::{Host, Location};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

/// What an explicit environment action asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Operation {
    /// Create, start and attach, running the setup that is due.
    Open,
    /// Replace the container with a fresh one.
    Rebuild,
    /// Attach to the existing container without setup.
    Reconnect,
    /// Release processes and forwards, then stop the container.
    Stop,
}

/// What became of one extension the declaration asked for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Extension {
    /// Already installed.
    Present(String),
    /// Installed from the catalogue for this declaration.
    Installed(String),
    /// Not installed and not installable, with the reason.
    Unavailable(String, String),
}

/// Something background environment work has to tell the window.
pub(super) enum EnvironmentBack {
    /// A progress event.
    Progress(Identity, Event),
    /// The environment is ready, with the outcome of its extension requests.
    Ready(Environment, Vec<Extension>),
    /// The environment was stopped.
    Stopped(Identity),
    /// The operation failed.
    Failed(Identity, Box<Failure>),
}

/// The window's dev container state.
pub(super) struct Environments {
    /// The provider, shared with the launched machines.
    pub docker: Docker,
    /// The progress notice of each operation under way, by checkout.
    working: BTreeMap<PathBuf, NoticeId>,
    /// The log of the latest operation, for the command that shows it.
    pub log: Option<PathBuf>,
}

impl Environments {
    /// The provider configured by `preferences`, keeping its records under the editor's home.
    pub(super) fn new(engine: &str) -> Self {
        let state = config::devcontainers_directory()
            .unwrap_or_else(|| PathBuf::from(".pandemonium/devcontainers"));
        Self {
            docker: Docker::new(Settings {
                engine: engine.to_owned(),
                state,
            }),
            working: BTreeMap::new(),
            log: None,
        }
    }
}

impl App {
    /// Runs an explicit environment action on the focused project or worktree.
    pub(super) fn environment_action(&mut self, operation: Operation) {
        let (identity, source) = match self.environment_subject() {
            Ok(subject) => subject,
            Err(error) => return self.notices.trouble(error, None),
        };
        let key = identity.checkout().to_path_buf();
        if self.environments.working.contains_key(&key) {
            return self
                .notices
                .trouble("An operation is already running on this environment", None);
        }
        let docker = self.environments.docker.clone();
        if matches!(operation, Operation::Open | Operation::Rebuild)
            && docker.declaration(&source).is_none()
            && self
                .environments
                .docker
                .records()
                .iter()
                .all(|record| record.identity != identity)
        {
            return self.notices.trouble(
                format!(
                    "{} has no .devcontainer/devcontainer.json",
                    source.display()
                ),
                None,
            );
        }
        self.environments.log = Some(docker.log(&identity));
        let id = self
            .notices
            .progress(format!("Dev container: {}", Phase::Checking.describe()));
        self.environments.working.insert(key, id);
        let back = self.remote_back.clone();
        let wake = self.waker(Wake::Remote);
        std::thread::spawn(move || {
            let events = back.clone();
            let progress = wake.clone();
            let reporting = identity.clone();
            let sink: pm_devcontainer::Sink = Arc::new(move |event| {
                events
                    .lock()
                    .unwrap()
                    .push(super::remote::RemoteBack::Environment(Box::new(
                        EnvironmentBack::Progress(reporting.clone(), event),
                    )));
                progress();
            });
            let result = match operation {
                Operation::Open | Operation::Rebuild => {
                    let intent = if operation == Operation::Open {
                        Intent::Open
                    } else {
                        Intent::Rebuild
                    };
                    let request = Request {
                        identity: identity.clone(),
                        source,
                        declaration: None,
                    };
                    docker.open(&request, intent, &sink).map(Some)
                }
                Operation::Reconnect => docker.reconnect(&identity, &sink).map(Some),
                Operation::Stop => docker.stop(&identity, &sink).map(|()| None),
            };
            let finished = match result {
                Ok(Some(environment)) => {
                    let extensions = resolve_extensions(environment.extensions());
                    EnvironmentBack::Ready(environment, extensions)
                }
                Ok(None) => EnvironmentBack::Stopped(identity),
                Err(failure) => EnvironmentBack::Failed(identity, Box::new(failure)),
            };
            back.lock()
                .unwrap()
                .push(super::remote::RemoteBack::Environment(Box::new(finished)));
            wake();
        });
    }

    /// The environment the focused project or worktree belongs to, and its folder on the desktop.
    fn environment_subject(&self) -> Result<(Identity, Location), String> {
        let scope = self.scope().ok_or("No project is open")?;
        let root = self.root_of(scope).ok_or("No project is open")?;
        if let Some(identity) = self.environments.docker.identity_of(&root.host) {
            let source = Location::local(identity.checkout());
            return Ok((identity, source));
        }
        if !root.host.is_local() {
            return Err("Dev containers open from a project on this machine".to_owned());
        }
        if scope.session().is_some() {
            return Err(
                "A session gets its own environment when its project is opened in a dev container"
                    .to_owned(),
            );
        }
        Ok((
            Identity::new(root.path.display().to_string(), &root.path),
            root,
        ))
    }

    /// Takes in what background environment work reported.
    pub(super) fn take_environment(&mut self, back: EnvironmentBack) {
        match back {
            EnvironmentBack::Progress(identity, event) => self.environment_event(&identity, event),
            EnvironmentBack::Ready(environment, extensions) => {
                self.environment_ready(environment, extensions)
            }
            EnvironmentBack::Stopped(identity) => {
                self.finish_environment_notice(&identity);
                self.notices
                    .done(format!("Dev container stopped: {}", identity.slug()), None);
            }
            EnvironmentBack::Failed(identity, failure) => {
                self.finish_environment_notice(&identity);
                self.notices
                    .trouble(failure_summary(&failure), Some(Message::OpenEnvironmentLog));
            }
        }
    }

    /// Shows the phase an operation reached.
    fn environment_event(&mut self, identity: &Identity, event: Event) {
        match event {
            Event::State(State::Working(phase)) => {
                if let Some(old) = self.environments.working.remove(identity.checkout()) {
                    self.notices.dismiss(old);
                }
                let id = self.notices.progress(format!(
                    "Dev container {}: {}",
                    identity.slug(),
                    phase.describe()
                ));
                self.environments
                    .working
                    .insert(identity.checkout().to_path_buf(), id);
            }
            Event::Notice(text) => self.notices.done(
                format!("Dev container: {text}"),
                Some(Message::OpenEnvironmentLog),
            ),
            Event::Forwards(forwards) => {
                for forward in forwards
                    .iter()
                    .filter(|forward| forward.remapped() || forward.local.is_err())
                {
                    let text = format!("Dev container: {}", forward.describe());
                    match forward.local {
                        Ok(_) => self.notices.done(text, None),
                        Err(_) => self
                            .notices
                            .trouble(text, Some(Message::OpenEnvironmentLog)),
                    }
                }
            }
            Event::State(_) => self.finish_environment_notice(identity),
            Event::Log { .. } => {}
        }
        self.request_redraw();
    }

    /// Clears the progress notice of an operation that ended.
    fn finish_environment_notice(&mut self, identity: &Identity) {
        if let Some(id) = self.environments.working.remove(identity.checkout()) {
            self.notices.dismiss(id);
        }
    }

    /// Opens the environment's workspace as a project, or refreshes the one already open.
    fn environment_ready(&mut self, environment: Environment, extensions: Vec<Extension>) {
        self.finish_environment_notice(environment.identity());
        self.environments.log = environment.log().map(std::path::Path::to_path_buf);
        let location = environment.workspace().clone();
        self.hosts.adopt(&location.host);
        location.host.set_notify(self.waker(Wake::Remote));
        let host = location.host.clone();
        if self
            .sessions
            .iter()
            .any(|session| session.root() == &location)
        {
            self.reconnected(host);
            self.notices.done(
                format!(
                    "Session environment ready: {}",
                    environment.identity().slug()
                ),
                None,
            );
            return self.report_extensions(extensions);
        }
        let already = self.open.iter().any(|project| project.root() == &location);
        match self.open.find_or_open(location) {
            Ok(project) => {
                if already {
                    self.reconnected(host);
                }
                self.point_at(Scope::checkout(project));
                self.watch_worktrees();
                self.store();
                self.notices.done(
                    format!("Dev container ready: {}", environment.identity().slug()),
                    None,
                );
            }
            Err(error) => self.notices.trouble(error.to_string(), None),
        }
        self.report_extensions(extensions);
    }

    /// Says what happened to each extension request, and activates what was installed.
    fn report_extensions(&mut self, extensions: Vec<Extension>) {
        let installed = extensions
            .iter()
            .any(|extension| matches!(extension, Extension::Installed(_)));
        let unavailable = extensions
            .iter()
            .filter_map(|extension| match extension {
                Extension::Unavailable(id, why) => Some(format!("{id} ({why})")),
                _ => None,
            })
            .collect::<Vec<_>>();
        if installed {
            for error in config::reload_extensions(&mut self.preferences) {
                self.notices.trouble(error, None);
            }
            self.activate_languages();
            let names = extensions
                .iter()
                .filter_map(|extension| match extension {
                    Extension::Installed(id) => Some(id.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(", ");
            self.notices
                .done(format!("Dev container installed extensions: {names}"), None);
        }
        if !unavailable.is_empty() {
            self.notices.trouble(
                format!(
                    "Dev container extensions unavailable: {}",
                    unavailable.join(", ")
                ),
                None,
            );
        }
    }

    /// Shows the log of the latest environment operation, where it grows as setup runs.
    pub(super) fn open_environment_log(&mut self) {
        match self.environments.log.clone() {
            Some(path) => self.open_server_log_at(&path),
            None => self
                .notices
                .trouble("No dev container has been opened in this window", None),
        }
    }

    /// Reconnects the machine of the active project when it is an environment.
    pub(super) fn reconnect_environment(&mut self, host: &Host) -> bool {
        if host.scheme() != Some("devcontainer") {
            return false;
        }
        self.environment_action(Operation::Reconnect);
        true
    }
}

/// One sentence for the status bar; the log holds the rest.
fn failure_summary(failure: &Failure) -> String {
    match failure.problems.0.as_slice() {
        [] => format!(
            "Dev container: {}",
            failure.message.lines().next().unwrap_or_default()
        ),
        problems => format!(
            "Dev container cannot be used as written: {}",
            problems
                .iter()
                .map(|problem| problem.field.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Resolves the extension identifiers through the installed set and the catalogue.
fn resolve_extensions(wanted: &[String]) -> Vec<Extension> {
    if wanted.is_empty() {
        return Vec::new();
    }
    let installed = config::extensions::installed();
    let present = |id: &String| installed.iter().any(|entry| &entry.id == id);
    let catalogue = if wanted.iter().all(present) {
        Ok(Vec::new())
    } else {
        config::extensions::catalogue()
    };
    wanted
        .iter()
        .map(|id| {
            if present(id) {
                return Extension::Present(id.clone());
            }
            match &catalogue {
                Ok(entries) => match entries.iter().find(|entry| &entry.id == id) {
                    Some(entry) => match config::extensions::install(entry) {
                        Ok(()) => Extension::Installed(id.clone()),
                        Err(error) => Extension::Unavailable(id.clone(), error),
                    },
                    None => Extension::Unavailable(id.clone(), "not in the catalogue".to_owned()),
                },
                Err(error) => Extension::Unavailable(id.clone(), error.clone()),
            }
        })
        .collect()
}

/// Gives session worktrees of native projects their own dev container.
struct Containers {
    /// The provider.
    docker: Docker,
    /// Where progress is reported, shared with the window.
    back: Arc<std::sync::Mutex<Vec<super::remote::RemoteBack>>>,
    /// Wakes the window when something was reported.
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl pm_core::Provision for Containers {
    /// Builds, starts and sets up the worktree's container before the session is taken up.
    fn isolate(&self, project: &Location, folder: &std::path::Path) -> std::io::Result<Location> {
        let identity = Identity::new(project.path.display().to_string(), folder);
        let reporting = identity.clone();
        let back = self.back.clone();
        let wake = self.wake.clone();
        let sink: pm_devcontainer::Sink = Arc::new(move |event| {
            back.lock()
                .unwrap()
                .push(super::remote::RemoteBack::Environment(Box::new(
                    EnvironmentBack::Progress(reporting.clone(), event),
                )));
            wake();
        });
        let request = Request {
            identity,
            source: Location::local(folder),
            declaration: None,
        };
        self.docker
            .open(&request, Intent::Open, &sink)
            .map(|environment| environment.workspace().clone())
            .map_err(|failure| std::io::Error::other(failure.to_string()))
    }

    /// The worktree's existing container, if this editor made one.
    fn locate(&self, project: &Location, folder: &std::path::Path) -> Option<Location> {
        self.docker
            .locate(&Identity::new(project.path.display().to_string(), folder))
    }
}

impl App {
    /// What gives a session's worktree its own dev container.
    pub(super) fn provision(&self) -> Arc<dyn pm_core::Provision> {
        Arc::new(Containers {
            docker: self.environments.docker.clone(),
            back: self.remote_back.clone(),
            wake: self.waker(Wake::Remote),
        })
    }

    /// Whether the session being started could run in a dev container: its project is
    /// on this machine and declares one.
    pub(super) fn session_could_use_container(&self) -> bool {
        self.open.active().is_some_and(|project| {
            let root = project.root();
            root.host.is_local() && self.environments.docker.declaration(root).is_some()
        })
    }
}
