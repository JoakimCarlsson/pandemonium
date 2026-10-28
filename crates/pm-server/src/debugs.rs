//! Programs the server debugs, kept going when no client is attached.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pm_dap::{Adapter, Breakpoint, Scenario, Session, Standing};
use pm_protocol::{
    DebugAdapter, DebugFile, DebugInfo, DebugProcess, DebugScenario, DebugView, ServerMessage,
};

use crate::core::{Connection, Core};

/// How long the server waits for more to arrive from the adapter before it tells the clients, so that a program printing a great deal costs them one message in a while rather than one for every line.
const GATHER: Duration = Duration::from_millis(30);

/// One program being debugged, and the connections told how it goes.
pub(crate) struct Debugging {
    /// The folder the program is debugged in, as the client that started it named it.
    directory: String,
    /// The session with the adapter.
    session: Arc<Session>,
    /// The connections told how the session goes.
    watchers: Mutex<Vec<Arc<Connection>>>,
}

impl Debugging {
    /// What the server tells a client about this session, numbered `debug`.
    fn info(&self, debug: u64) -> DebugInfo {
        DebugInfo {
            debug,
            directory: self.directory.clone(),
            label: self.session.scenario().label.clone(),
        }
    }

    /// Subscribes `connection`, once.
    fn watch(&self, connection: &Arc<Connection>) {
        if let Ok(mut watchers) = self.watchers.lock()
            && !watchers.iter().any(|held| Arc::ptr_eq(held, connection))
        {
            watchers.push(connection.clone());
        }
    }

    /// Tells the connections watching what the session looks like now and what happened since they were last told.
    fn tell(&self, debug: u64) {
        let message = ServerMessage::Debug {
            debug,
            view: self.session.view(),
            events: self.session.take_events(),
        };
        if let Ok(mut watchers) = self.watchers.lock() {
            watchers.retain(|connection| connection.outbox.send(message.clone()).is_ok());
        }
    }
}

/// The breakpoints `files` set in the folder `root`, by absolute path.
fn breakpoints_in(
    root: &Path,
    files: Vec<DebugFile>,
) -> std::collections::BTreeMap<PathBuf, Vec<Breakpoint>> {
    files
        .into_iter()
        .map(|file| (root.join(file.path), file.breakpoints))
        .collect()
}

/// What a client is told about `scenario`.
fn described(scenario: &Scenario) -> DebugScenario {
    DebugScenario {
        label: scenario.label.clone(),
        adapter: scenario
            .adapter
            .as_ref()
            .map_or_else(|| scenario.kind.clone(), |adapter| adapter.name.clone()),
        installed: scenario.adapter.as_ref().is_some_and(Adapter::installed),
        attach: scenario.request == pm_dap::Request::Attach,
        source: scenario
            .source
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        task: scenario.before.clone(),
    }
}

/// Every scenario of the folder `root` with the file in front, `file` from its root.
fn scenarios_of(root: &Path, file: Option<&str>) -> Vec<Scenario> {
    pm_dap::scenarios(root, file.map(|file| root.join(file)).as_deref())
}

/// What the folder `root` can be debugged as, with the file in front, `file` from its root.
pub(crate) fn scenarios(root: &Path, file: Option<&str>) -> Vec<DebugScenario> {
    scenarios_of(root, file).iter().map(described).collect()
}

/// The programs running here, and the adapters installed that could attach to one.
pub(crate) fn processes() -> (Vec<DebugAdapter>, Vec<DebugProcess>) {
    let adapters = pm_dap::adapters()
        .into_iter()
        .filter(|adapter| adapter.denied.is_none() && adapter.installed())
        .map(|adapter| DebugAdapter {
            id: adapter.id,
            name: adapter.name,
            attach_names: adapter.attach_names,
        })
        .collect();
    let processes = pm_dap::processes()
        .into_iter()
        .map(|process| DebugProcess {
            pid: process.pid,
            name: process.name,
            command: process.command,
        })
        .collect();
    (adapters, processes)
}

/// Tells the watchers each time the session moves, until it ends, and then lets it go.
fn pump(core: Arc<Core>, debug: u64, debugging: Arc<Debugging>, woken: Receiver<()>) {
    while woken.recv().is_ok() {
        std::thread::sleep(GATHER);
        while woken.try_recv().is_ok() {}
        if !debugging.session.take_fresh() {
            continue;
        }
        debugging.tell(debug);
        if debugging.session.standing() == Standing::Ended {
            break;
        }
    }
    if let Ok(mut held) = core.debugs.lock() {
        held.remove(&debug);
    }
}

impl Core {
    /// Starts debugging `scenario` in `root`, which the client named `directory`, watched by `connection`, and returns its number.
    pub(crate) fn start_debug(
        self: &Arc<Self>,
        connection: &Arc<Connection>,
        directory: &str,
        root: &Path,
        scenario: Scenario,
        files: Vec<DebugFile>,
    ) -> Result<(u64, DebugView), String> {
        let (wake, woken) = mpsc::channel();
        let wake = Mutex::new(wake);
        let notify: pm_dap::Notify = Arc::new(move || {
            if let Ok(wake) = wake.lock() {
                let _ = wake.send(());
            }
        });
        let session = Session::start(scenario, root, breakpoints_in(root, files), notify)
            .map_err(|error| error.to_string())?;
        let debugging = Arc::new(Debugging {
            directory: directory.to_owned(),
            session: Arc::new(session),
            watchers: Mutex::new(vec![connection.clone()]),
        });
        let debug = self.next_debug();
        let view = debugging.session.view();
        if let Ok(mut held) = self.debugs.lock() {
            held.insert(debug, debugging.clone());
        }
        let core = self.clone();
        std::thread::spawn(move || pump(core, debug, debugging, woken));
        Ok((debug, view))
    }

    /// Starts debugging in `directory`, part of the project `project`, as the scenario at `index` of what `file` makes of it, once the task it names has finished; nothing starts when that task fails.
    pub(crate) fn debug_scenario(
        self: &Arc<Self>,
        connection: &Arc<Connection>,
        directory: &str,
        project: Option<&str>,
        file: Option<&str>,
        index: usize,
        files: Vec<DebugFile>,
    ) -> Result<(u64, DebugView), String> {
        let root = crate::handler::place(directory);
        let root = root.as_path();
        let scenario = scenarios_of(root, file)
            .into_iter()
            .nth(index)
            .ok_or_else(|| "that is not something the folder can be debugged as".to_owned())?;
        if let Some(task) = &scenario.before {
            crate::tasks::finish_named(root, project, task)?;
        }
        self.start_debug(connection, directory, root, scenario, files)
    }

    /// Starts debugging the process `pid` in `directory` with the adapter called `adapter`.
    pub(crate) fn debug_attach(
        self: &Arc<Self>,
        connection: &Arc<Connection>,
        directory: &str,
        root: &Path,
        adapter: &str,
        pid: u32,
        files: Vec<DebugFile>,
    ) -> Result<(u64, DebugView), String> {
        let adapter = pm_dap::Adapter::with_id(adapter)
            .ok_or_else(|| format!("there is no debug adapter called {adapter}"))?;
        self.start_debug(connection, directory, root, adapter.attach(pid), files)
    }

    /// The debugging session numbered `number`, if the server still holds it.
    fn debugging(&self, number: u64) -> Option<Arc<Debugging>> {
        self.debugs.lock().ok()?.get(&number).cloned()
    }

    /// Subscribes `connection` to the session numbered `number` and says what it looks like; nothing when there is none.
    pub(crate) fn watch_debug(
        &self,
        connection: &Arc<Connection>,
        number: u64,
    ) -> Option<DebugView> {
        let debugging = self.debugging(number)?;
        debugging.watch(connection);
        Some(debugging.session.view())
    }

    /// Does `action` to the session numbered `number`; false when there is no such session.
    pub(crate) fn debug_do(&self, number: u64, action: pm_protocol::DebugAction) -> bool {
        use pm_protocol::DebugAction as Do;
        let Some(debugging) = self.debugging(number) else {
            return false;
        };
        let session = &debugging.session;
        match action {
            Do::Continue => session.resume(),
            Do::Pause => session.pause(),
            Do::StepOver => session.step_over(),
            Do::StepIn => session.step_in(),
            Do::StepOut => session.step_out(),
            Do::Stop => session.stop(),
            Do::SelectFrame { frame } => session.select_frame(frame),
            Do::Expand { reference } => session.expand(reference),
            Do::Evaluate { expression } => session.evaluate(&expression),
            Do::SetWatches { watches } => session.set_watches(watches),
            Do::SetBreakpoints { path, breakpoints } => {
                let root = crate::handler::place(&debugging.directory);
                session.set_breakpoints(&root.join(path), breakpoints);
            }
        }
        true
    }

    /// What the server knows about each debugging session, oldest first.
    pub(crate) fn debug_infos(&self) -> Vec<DebugInfo> {
        let Ok(held) = self.debugs.lock() else {
            return Vec::new();
        };
        let mut infos: Vec<DebugInfo> = held
            .iter()
            .map(|(number, debugging)| debugging.info(*number))
            .collect();
        infos.sort_by_key(|info| info.debug);
        infos
    }
}

/// The debug adapters and the offers of what is installed on this machine: those the installed languages declare, then those the extensions bring.
pub(crate) struct Installed;

impl pm_dap::Catalog for Installed {
    fn adapters(&self) -> Vec<Adapter> {
        let owners = crate::extensions::language_owners();
        let mut languages = pm_lsp::installed();
        languages.sort_by_key(|(id, _)| owners.contains_key(id));
        let mut found: Vec<Adapter> = Vec::new();
        for (id, manifest) in languages {
            let denied = owners.get(&id).and_then(|package| {
                crate::extensions::grants_of(&package.id)
                    .check(pm_policy::Capability::Process)
                    .err()
                    .map(|denied| denied.to_string())
            });
            for spec in manifest.debug_adapters {
                let types: Vec<String> =
                    spec.types.iter().map(|name| name.to_lowercase()).collect();
                if found.iter().any(|held| {
                    held.id == spec.id || held.types.iter().any(|name| types.contains(name))
                }) {
                    continue;
                }
                found.push(Adapter {
                    id: spec.id,
                    name: spec.name,
                    programs: spec.programs,
                    arguments: spec.arguments,
                    connect: if spec.connect == "tcp" {
                        pm_dap::Connect::Tcp
                    } else {
                        pm_dap::Connect::Stdio
                    },
                    types,
                    denied: denied.clone(),
                    provided: false,
                    attach_names: spec.attach_names,
                    set: spec.set,
                    default: spec.default,
                });
            }
        }
        for adapter in crate::extensions::adapters() {
            if !found.iter().any(|held| {
                held.id == adapter.id || held.types.iter().any(|name| adapter.types.contains(name))
            }) {
                found.push(adapter);
            }
        }
        found
    }

    fn offers(&self, name: &str) -> Vec<pm_dap::Offer> {
        pm_lsp::installed()
            .into_iter()
            .filter(|(_, manifest)| manifest.matches(name))
            .flat_map(|(_, manifest)| manifest.debug_scenarios)
            .filter(|spec| {
                name.rsplit_once('.').is_some_and(|(_, suffix)| {
                    spec.suffixes
                        .iter()
                        .any(|held| held.eq_ignore_ascii_case(suffix))
                })
            })
            .map(|spec| pm_dap::Offer {
                label: spec.label,
                kind: spec.adapter,
                config: spec.config,
            })
            .collect()
    }
}
