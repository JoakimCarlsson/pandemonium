//! What the link keeps for the dock: the shells of each folder with the one
//! shown, the problems the language servers report, the tasks of the folder
//! followed and the debugging sessions with the breakpoints set. Every
//! change it needs from the server is queued as a request the link sends.

use std::collections::{BTreeMap, HashMap, HashSet};

use pm_protocol::{
    Breakpoint, DebugAction, DebugEvent, DebugFile, DebugView, Request, Response, Standing, Task,
};

use crate::dock_model::{
    DebugBreakpoint, DebugCommand, DebugFrameEntry, DebugModel, DebugScenarioEntry,
    DebugScopeEntry, DebugSession, DebugStanding, DebugVariableEntry, InlineValue, Problem,
    ProblemSeverity, TaskEntry, TaskList, TerminalTab,
};
use crate::model::Workspace;
use crate::terminal::{START_COLUMNS, START_ROWS, Shell};

/// The folder a shell starts in before a session is open: the home folder of the machine the server runs on.
pub const HOME: &str = "~/";

/// How many console lines the Debug tab keeps.
const CONSOLE_KEPT: usize = 200;

/// What an outstanding request of the dock was for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DockAsk {
    /// The shell numbered this, started for a tab.
    Shell(u64),
    /// The tasks of this folder.
    Tasks(String),
    /// What this folder can be debugged as.
    Scenarios(String),
    /// A debugging session started.
    DebugStart,
    /// A debugging session subscribed to.
    DebugWatch,
    /// The debugging sessions the server holds.
    Debugs,
    /// Nothing that needs handling.
    Nothing,
}

/// A debugging session the server told of, with what it looks like now.
struct Debugged {
    /// The folder it debugs.
    folder: String,
    /// What it looks like now.
    view: DebugView,
}

/// The dock's state behind the workspace it publishes.
pub struct Dock {
    /// Whether the server is reached.
    connected: bool,
    /// The folder the dock follows: the open session's, or the home folder.
    place: String,
    /// Every shell asked for, in the order they were.
    shells: Vec<Shell>,
    /// The number the next shell is kept by.
    next_key: u64,
    /// The shell shown for each folder, by its key.
    shown: HashMap<String, u64>,
    /// Output of shells not yet known by number, kept until their number arrives.
    orphans: HashMap<u64, Vec<u8>>,
    /// How many characters wide and lines tall the dock's terminal is.
    size: (u16, u16),
    /// What each opened file's language server finds wrong, by folder and file.
    diagnostics: BTreeMap<(String, String), Vec<Problem>>,
    /// The tasks the server listed for the place.
    tasks: Vec<Task>,
    /// The tasks tab as the pages draw it.
    task_list: TaskList,
    /// What the place can be debugged as, as the server listed it.
    scenarios: Vec<DebugScenarioEntry>,
    /// Whether the scenarios of the place were answered.
    scenarios_loaded: bool,
    /// Every debugging session told of, by its number.
    debugs: HashMap<u64, Debugged>,
    /// The session the Debug tab follows.
    followed: Option<u64>,
    /// The variables whose members are shown, by reference.
    expanded: HashSet<i64>,
    /// The references asked about since the program last paused.
    asked: HashSet<i64>,
    /// The breakpoints set, by file and line.
    breakpoints: BTreeMap<String, Vec<usize>>,
    /// Why the last start of a debugging session failed.
    debug_error: Option<String>,
    /// The requests to send, with what each is for.
    outbox: Vec<(Request, DockAsk)>,
    /// How many lines each shell keeps above its window.
    scrollback: usize,
}

impl Default for Dock {
    /// A dock following the home folder with nothing asked yet.
    fn default() -> Self {
        Self {
            connected: false,
            place: HOME.to_owned(),
            shells: Vec::new(),
            next_key: 1,
            shown: HashMap::new(),
            orphans: HashMap::new(),
            size: (START_COLUMNS, START_ROWS),
            diagnostics: BTreeMap::new(),
            tasks: Vec::new(),
            task_list: TaskList::default(),
            scenarios: Vec::new(),
            scenarios_loaded: false,
            debugs: HashMap::new(),
            followed: None,
            expanded: HashSet::new(),
            asked: HashSet::new(),
            breakpoints: BTreeMap::new(),
            debug_error: None,
            outbox: Vec::new(),
            scrollback: pm_config::SCROLLBACK,
        }
    }
}

impl Dock {
    /// Has every shell, and those started later, keep `lines` above its window.
    pub fn set_scrollback(&mut self, lines: usize) {
        if lines == self.scrollback {
            return;
        }
        self.scrollback = lines;
        for shell in &mut self.shells {
            shell.emulator.set_scrollback(lines);
        }
    }

    /// Takes the requests queued since the last call, with what each is for.
    pub fn take_requests(&mut self) -> Vec<(Request, DockAsk)> {
        std::mem::take(&mut self.outbox)
    }

    /// Queues `request`, remembering it was for `ask`.
    fn send(&mut self, request: Request, ask: DockAsk) {
        if self.connected {
            self.outbox.push((request, ask));
        }
    }

    /// Notes the server is reached: starts the place's shell, and asks for the debugging sessions running and what the place offers.
    pub fn connected(&mut self) {
        self.connected = true;
        self.send(Request::Debugs, DockAsk::Debugs);
        self.ensure_shell();
        self.ask_place();
    }

    /// Follows `folder`, starting a shell there unless one runs and asking what it offers when it changed.
    pub fn follow(&mut self, folder: String) {
        let changed = folder != self.place;
        self.place = folder;
        self.ensure_shell();
        if changed {
            self.ask_place();
        }
    }

    /// Moves the shells of the place to `folder`, keeping the shell shown, as starting a session from the start page hands its shells to the session; nothing when `folder` holds shells of its own.
    pub fn hand_over(&mut self, folder: String) {
        if folder == self.place || self.shells.iter().any(|shell| shell.folder == folder) {
            return;
        }
        let place = self.place.clone();
        for shell in self.shells.iter_mut().filter(|shell| shell.folder == place) {
            shell.folder.clone_from(&folder);
        }
        if let Some(key) = self.shown.remove(&place) {
            self.shown.insert(folder, key);
        }
    }

    /// The folder the dock follows: the open session's, or the home folder.
    pub fn place(&self) -> &str {
        &self.place
    }

    /// Asks the server for the tasks and debug scenarios of the place, when a session gives it one.
    fn ask_place(&mut self) {
        self.tasks.clear();
        self.task_list = TaskList {
            available: self.place != HOME,
            ..TaskList::default()
        };
        self.scenarios.clear();
        self.scenarios_loaded = false;
        if self.place == HOME {
            return;
        }
        let directory = self.place.clone();
        self.send(
            Request::Tasks {
                directory: directory.clone(),
            },
            DockAsk::Tasks(directory.clone()),
        );
        self.send(
            Request::DebugScenarios {
                directory: directory.clone(),
                file: None,
            },
            DockAsk::Scenarios(directory),
        );
    }

    /// Asks for the tasks and debug scenarios of the place again.
    pub fn refresh(&mut self) {
        self.ask_place();
    }

    /// The shells of the place, in order.
    fn place_shells(&self) -> impl Iterator<Item = &Shell> {
        self.shells
            .iter()
            .filter(move |shell| shell.folder == self.place)
    }

    /// The shell the dock shows for the place.
    fn current(&mut self) -> Option<&mut Shell> {
        let key = self.shown.get(&self.place).copied()?;
        self.shells.iter_mut().find(|shell| shell.key == key)
    }

    /// Asks the server for a shell in the place unless one was asked for already.
    fn ensure_shell(&mut self) {
        if self.connected && self.place_shells().next().is_none() {
            self.new_shell();
        }
    }

    /// Asks for another shell in the place and shows it.
    pub fn new_shell(&mut self) {
        let key = self.add_shell(None);
        let (columns, rows) = self.size;
        self.send(
            Request::OpenTerminal {
                directory: self.place.clone(),
                columns,
                rows,
            },
            DockAsk::Shell(key),
        );
    }

    /// Adds a shell in the place, running `task` when given, shows it and returns its key.
    fn add_shell(&mut self, task: Option<String>) -> u64 {
        let key = self.next_key;
        self.next_key += 1;
        let (columns, rows) = self.size;
        let mut shell = Shell::starting(key, self.place.clone(), columns, rows);
        shell.emulator.set_scrollback(self.scrollback);
        shell.task = task;
        self.shells.push(shell);
        self.shown.insert(self.place.clone(), key);
        key
    }

    /// Shows the shell kept as `key`.
    pub fn pick(&mut self, key: u64) {
        if let Some(shell) = self.shells.iter().find(|shell| shell.key == key) {
            self.shown.insert(shell.folder.clone(), key);
        }
    }

    /// Ends the shell kept as `key` and drops its tab, showing its neighbour.
    pub fn close(&mut self, key: u64) {
        let Some(index) = self.shells.iter().position(|shell| shell.key == key) else {
            return;
        };
        let shell = self.shells.remove(index);
        if let Some(terminal) = shell.id.filter(|_| !shell.exited) {
            self.send(Request::CloseTerminal { terminal }, DockAsk::Nothing);
        }
        if self.shown.get(&shell.folder) == Some(&key) {
            let neighbour = self
                .shells
                .iter()
                .filter(|other| other.folder == shell.folder)
                .take(index.max(1))
                .last()
                .map(|other| other.key);
            match neighbour {
                Some(neighbour) => self.shown.insert(shell.folder, neighbour),
                None => self.shown.remove(&shell.folder),
            };
        }
    }

    /// Types `data` into the shown shell, scrolled to the end, starting a new shell in its place when it ended.
    pub fn input(&mut self, data: Vec<u8>) {
        let Some(shell) = self.current() else {
            self.ensure_shell();
            return;
        };
        if shell.exited {
            let key = shell.key;
            self.close(key);
            self.new_shell();
            return;
        }
        shell.back = 0;
        if let Some(terminal) = shell.id {
            self.send(Request::TerminalInput { terminal, data }, DockAsk::Nothing);
        }
    }

    /// Scrolls the shown shell `lines` back, when positive, or forward; a full-screen program gets arrow keys instead.
    pub fn scroll(&mut self, lines: i32) {
        let Some(shell) = self.current() else {
            return;
        };
        if shell.emulator.on_alternate_screen() {
            let data = shell.wheel_bytes(lines);
            if let Some(terminal) = shell.id {
                self.send(Request::TerminalInput { terminal, data }, DockAsk::Nothing);
            }
        } else {
            shell.scroll(lines);
        }
    }

    /// Marks text of the shown shell from `anchor` to `head` in lines of its window, by cells, words or lines as `clicks` says.
    pub fn select(&mut self, anchor: (usize, usize), head: (usize, usize), clicks: u32) {
        if let Some(shell) = self.current() {
            shell.select(anchor, head, clicks);
        }
    }

    /// The text marked in the shown shell.
    pub fn selected_text(&mut self) -> Option<String> {
        self.current()?.emulator.selected_text()
    }

    /// Types `text` into the shown shell as a paste.
    pub fn paste(&mut self, text: &str) {
        let Some(shell) = self.current() else {
            return;
        };
        let data = shell.paste_bytes(text);
        self.input(data);
    }

    /// Marks everything the shown shell keeps.
    pub fn select_all(&mut self) {
        if let Some(shell) = self.current() {
            shell.emulator.select_all();
        }
    }

    /// Clears the shown shell's window and what it keeps above it.
    pub fn clear(&mut self) {
        if let Some(shell) = self.current() {
            shell.emulator.clear();
            shell.back = 0;
        }
    }

    /// Makes every shell `columns` characters wide and `rows` lines tall and returns whether that changed anything.
    pub fn resize(&mut self, columns: u16, rows: u16) -> bool {
        let columns = columns.max(2);
        let rows = rows.max(1);
        if self.size == (columns, rows) {
            return false;
        }
        self.size = (columns, rows);
        let started: Vec<u64> = self
            .shells
            .iter_mut()
            .filter_map(|shell| {
                shell.emulator.resize(columns.into(), rows.into());
                shell.back = shell.back.min(shell.emulator.history());
                shell.id
            })
            .collect();
        for terminal in started {
            self.send(
                Request::ResizeTerminal {
                    terminal,
                    columns,
                    rows,
                },
                DockAsk::Nothing,
            );
        }
        true
    }

    /// Ends every shell and forgets them, for a link that only looked.
    pub fn close_all(&mut self) {
        let started: Vec<u64> = self.shells.drain(..).filter_map(|shell| shell.id).collect();
        self.shown.clear();
        for terminal in started {
            self.send(Request::CloseTerminal { terminal }, DockAsk::Nothing);
        }
    }

    /// Records that the shell kept as `key` is numbered `terminal`, and feeds it what it printed meanwhile.
    fn shell_started(&mut self, key: u64, terminal: u64) {
        let early = self.orphans.remove(&terminal);
        if let Some(shell) = self.shells.iter_mut().find(|shell| shell.key == key) {
            shell.id = Some(terminal);
        }
        if let Some(early) = early {
            self.output(terminal, &early);
        }
    }

    /// Feeds `data`, printed by the shell numbered `terminal`, to its emulator and answers what the emulator replies.
    pub fn output(&mut self, terminal: u64, data: &[u8]) {
        let Some(shell) = self
            .shells
            .iter_mut()
            .find(|shell| shell.id == Some(terminal))
        else {
            self.orphans
                .entry(terminal)
                .or_default()
                .extend_from_slice(data);
            return;
        };
        shell.feed(data);
        let replies = shell.emulator.take_replies();
        if !replies.is_empty() {
            self.send(
                Request::TerminalInput {
                    terminal,
                    data: replies,
                },
                DockAsk::Nothing,
            );
        }
    }

    /// Notes the shell numbered `terminal` ended.
    pub fn exited(&mut self, terminal: u64) {
        if let Some(shell) = self
            .shells
            .iter_mut()
            .find(|shell| shell.id == Some(terminal))
        {
            shell.exited = true;
        }
    }

    /// Takes what a language server finds wrong with `path` in `folder`, replacing what it found before.
    pub fn diagnostics(
        &mut self,
        folder: String,
        path: String,
        found: Vec<pm_protocol::Diagnostic>,
    ) {
        let problems: Vec<Problem> = found
            .into_iter()
            .map(|diagnostic| Problem {
                folder: folder.clone(),
                path: path.clone(),
                line: diagnostic.span.start.line as usize,
                column: diagnostic.span.start.character as usize,
                severity: match diagnostic.severity {
                    pm_protocol::Severity::Error => ProblemSeverity::Error,
                    pm_protocol::Severity::Warning => ProblemSeverity::Warning,
                    pm_protocol::Severity::Information => ProblemSeverity::Information,
                    pm_protocol::Severity::Hint => ProblemSeverity::Hint,
                },
                message: diagnostic.message,
                source: diagnostic.source,
            })
            .collect();
        if problems.is_empty() {
            self.diagnostics.remove(&(folder, path));
        } else {
            self.diagnostics.insert((folder, path), problems);
        }
    }

    /// Runs the task at `index` of the place's list in a new shell tab.
    pub fn run_task(&mut self, index: usize) {
        let Some(task) = self.tasks.get(index).cloned() else {
            return;
        };
        let key = self.add_shell(Some(task.name.clone()));
        let (columns, rows) = self.size;
        self.send(
            Request::RunTask {
                directory: self.place.clone(),
                project: None,
                task: Box::new(task),
                columns,
                rows,
            },
            DockAsk::Shell(key),
        );
    }

    /// Starts debugging the place as the scenario at `index`, with the breakpoints set.
    pub fn debug_start(&mut self, index: usize) {
        self.debug_error = None;
        let files = self.debug_files();
        self.send(
            Request::DebugStart {
                directory: self.place.clone(),
                file: None,
                scenario: index,
                files,
                project: None,
            },
            DockAsk::DebugStart,
        );
    }

    /// The breakpoints set, file by file, as the server takes them.
    fn debug_files(&self) -> Vec<DebugFile> {
        self.breakpoints
            .iter()
            .map(|(path, lines)| DebugFile {
                path: path.clone(),
                breakpoints: lines.iter().map(|&line| breakpoint(line)).collect(),
            })
            .collect()
    }

    /// Does `command` to the session followed.
    pub fn debug_command(&mut self, command: DebugCommand) {
        let Some(debug) = self.followed else {
            return;
        };
        let action = match command {
            DebugCommand::Continue => DebugAction::Continue,
            DebugCommand::Pause => DebugAction::Pause,
            DebugCommand::StepOver => DebugAction::StepOver,
            DebugCommand::StepIn => DebugAction::StepIn,
            DebugCommand::StepOut => DebugAction::StepOut,
            DebugCommand::Stop => DebugAction::Stop,
            DebugCommand::SelectFrame(frame) => DebugAction::SelectFrame { frame },
            DebugCommand::Evaluate(expression) => DebugAction::Evaluate { expression },
            DebugCommand::Expand(reference) => {
                if !self.expanded.remove(&reference) {
                    self.expanded.insert(reference);
                    self.ask_variables(debug, reference);
                }
                return;
            }
        };
        self.send(Request::DebugDo { debug, action }, DockAsk::Nothing);
    }

    /// Asks the session `debug` for the variables behind `reference` unless they are known or asked for.
    fn ask_variables(&mut self, debug: u64, reference: i64) {
        let known = self
            .debugs
            .get(&debug)
            .is_some_and(|debugged| debugged.view.variables_of(reference).is_some());
        if known || !self.asked.insert(reference) {
            return;
        }
        self.send(
            Request::DebugDo {
                debug,
                action: DebugAction::Expand { reference },
            },
            DockAsk::Nothing,
        );
    }

    /// Sets a breakpoint on `line` of `path`, or takes the one there away, and tells the session followed.
    pub fn toggle_breakpoint(&mut self, path: String, line: usize) {
        let lines = self.breakpoints.entry(path.clone()).or_default();
        match lines.binary_search(&line) {
            Ok(at) => {
                lines.remove(at);
            }
            Err(at) => lines.insert(at, line),
        }
        let lines = lines.clone();
        if lines.is_empty() {
            self.breakpoints.remove(&path);
        }
        let running = self.followed.filter(|debug| {
            self.debugs
                .get(debug)
                .is_some_and(|debugged| debugged.view.standing != Standing::Ended)
        });
        if let Some(debug) = running {
            self.send(
                Request::DebugDo {
                    debug,
                    action: DebugAction::SetBreakpoints {
                        path,
                        breakpoints: lines.into_iter().map(breakpoint).collect(),
                    },
                },
                DockAsk::Nothing,
            );
        }
    }

    /// Takes in what the session `debug` looks like now and what happened in it.
    pub fn debug_update(&mut self, debug: u64, view: DebugView, events: Vec<DebugEvent>) {
        if events
            .iter()
            .any(|event| matches!(event, DebugEvent::Paused(_)))
        {
            self.asked.clear();
            self.expanded.clear();
        }
        for event in &events {
            if let DebugEvent::StartRefused(reason) = event {
                self.debug_error = Some(reason.clone());
            }
        }
        let folder = self
            .debugs
            .get(&debug)
            .map(|debugged| debugged.folder.clone())
            .unwrap_or_else(|| self.place.clone());
        self.take_view(debug, folder, view);
    }

    /// Keeps `view` for the session `debug` in `folder`, follows it when none is, and asks for the variables of its scopes.
    fn take_view(&mut self, debug: u64, folder: String, view: DebugView) {
        let scopes: Vec<i64> = view
            .scopes
            .iter()
            .filter(|scope| !scope.expensive)
            .map(|scope| scope.reference)
            .collect();
        let stopped = view.standing == Standing::Stopped;
        self.debugs.insert(debug, Debugged { folder, view });
        if self.followed.is_none() || self.followed == Some(debug) {
            self.followed = Some(debug);
            if stopped {
                for reference in scopes {
                    self.ask_variables(debug, reference);
                }
            }
        }
    }

    /// Handles the `response` to a request made for `ask`.
    pub fn answer(&mut self, ask: DockAsk, response: Response) {
        match (ask, response) {
            (DockAsk::Shell(key), Response::Terminal { terminal }) => {
                self.shell_started(key, terminal)
            }
            (DockAsk::Shell(key), Response::Failed { message }) => {
                if let Some(shell) = self.shells.iter_mut().find(|shell| shell.key == key) {
                    shell.exited = true;
                    shell.feed(message.as_bytes());
                }
            }
            (DockAsk::Tasks(folder), Response::Tasks { tasks, problem })
                if folder == self.place =>
            {
                self.task_list = TaskList {
                    loaded: true,
                    available: true,
                    tasks: tasks
                        .iter()
                        .map(|task| TaskEntry {
                            name: task.name.clone(),
                            command: task.command.clone(),
                            group: task.group.clone(),
                            source: task.source.clone(),
                        })
                        .collect(),
                    problem,
                };
                self.tasks = tasks;
            }
            (DockAsk::Tasks(folder), Response::Failed { message }) if folder == self.place => {
                self.task_list = TaskList {
                    loaded: true,
                    available: true,
                    tasks: Vec::new(),
                    problem: Some(message),
                };
            }
            (DockAsk::Scenarios(folder), Response::DebugScenarios { scenarios })
                if folder == self.place =>
            {
                self.scenarios = scenarios
                    .into_iter()
                    .map(|scenario| DebugScenarioEntry {
                        label: scenario.label,
                        adapter: scenario.adapter,
                        installed: scenario.installed,
                    })
                    .collect();
                self.scenarios_loaded = true;
            }
            (DockAsk::Scenarios(folder), Response::Failed { message }) if folder == self.place => {
                self.scenarios_loaded = true;
                self.debug_error = Some(message);
            }
            (DockAsk::DebugStart, Response::Debug { debug, view }) => {
                self.followed = Some(debug);
                self.expanded.clear();
                self.asked.clear();
                let folder = self.place.clone();
                self.take_view(debug, folder, view);
            }
            (DockAsk::DebugStart, Response::Failed { message }) => self.debug_error = Some(message),
            (DockAsk::DebugWatch, Response::Debug { debug, view }) => {
                let folder = self
                    .debugs
                    .get(&debug)
                    .map(|debugged| debugged.folder.clone())
                    .unwrap_or_else(|| self.place.clone());
                self.take_view(debug, folder, view);
            }
            (DockAsk::Debugs, Response::Debugs { debugs }) => {
                for info in debugs {
                    self.send(
                        Request::DebugWatch { debug: info.debug },
                        DockAsk::DebugWatch,
                    );
                    self.debugs.entry(info.debug).or_insert(Debugged {
                        folder: info.directory,
                        view: empty_view(info.label),
                    });
                }
            }
            _ => {}
        }
    }

    /// Puts what the dock shows into `workspace`.
    pub fn publish(&self, workspace: &mut Workspace) {
        let shown = self.shown.get(&self.place).copied();
        workspace.terminals = self
            .place_shells()
            .enumerate()
            .map(|(index, shell)| TerminalTab {
                key: shell.key,
                title: shell_title(shell, index),
                exited: shell.exited,
                task: shell.task.is_some(),
            })
            .collect();
        workspace.terminal_shown = shown;
        workspace.terminal = self
            .shells
            .iter()
            .find(|shell| Some(shell.key) == shown)
            .map(Shell::screen)
            .unwrap_or_default();
        let mut problems: Vec<Problem> = self.diagnostics.values().flatten().cloned().collect();
        problems.sort_by(|left, right| {
            (left.severity, &left.path, left.line).cmp(&(right.severity, &right.path, right.line))
        });
        workspace.problems = problems;
        workspace.tasks = self.task_list.clone();
        workspace.debug = self.debug_model();
    }

    /// The Debug tab as the pages draw it.
    fn debug_model(&self) -> DebugModel {
        let session = self
            .followed
            .and_then(|debug| self.debugs.get(&debug).map(|debugged| (debug, debugged)))
            .map(|(debug, debugged)| self.session_model(debug, &debugged.view));
        let placed = self
            .followed
            .and_then(|debug| self.debugs.get(&debug))
            .map(|debugged| &debugged.view);
        DebugModel {
            loaded: self.scenarios_loaded,
            scenarios: self.scenarios.clone(),
            session,
            breakpoints: self
                .breakpoints
                .iter()
                .flat_map(|(path, lines)| {
                    lines.iter().map(move |&line| DebugBreakpoint {
                        path: path.clone(),
                        line,
                        verified: placed.and_then(|view| verified(view, path, line)),
                    })
                })
                .collect(),
            error: self.debug_error.clone(),
        }
    }

    /// The session numbered `debug` that looks like `view`, as the Debug tab draws it.
    fn session_model(&self, debug: u64, view: &DebugView) -> DebugSession {
        let scopes: Vec<DebugScopeEntry> = view
            .scopes
            .iter()
            .map(|scope| {
                let mut variables = Vec::new();
                self.flatten(view, scope.reference, 0, &mut variables);
                DebugScopeEntry {
                    name: scope.name.clone(),
                    reference: scope.reference,
                    variables,
                }
            })
            .collect();
        let inline_values = view
            .selected_frame()
            .or(view.frames.first())
            .filter(|_| view.standing == Standing::Stopped)
            .and_then(|frame| {
                let path = frame.path.as_ref()?;
                let text = scopes
                    .first()?
                    .variables
                    .iter()
                    .filter(|variable| variable.depth == 0)
                    .take(6)
                    .map(|variable| format!("{} = {}", variable.name, variable.value))
                    .collect::<Vec<_>>()
                    .join(", ");
                (!text.is_empty()).then(|| InlineValue {
                    path: path.to_string_lossy().into_owned(),
                    line: frame.line,
                    text,
                })
            })
            .into_iter()
            .collect();
        DebugSession {
            id: debug,
            label: view.label.clone(),
            standing: match view.standing {
                Standing::Starting => DebugStanding::Starting,
                Standing::Running => DebugStanding::Running,
                Standing::Stopped => DebugStanding::Stopped,
                Standing::Ended => DebugStanding::Ended,
            },
            reason: view.reason.clone(),
            frames: view
                .frames
                .iter()
                .map(|frame| DebugFrameEntry {
                    id: frame.id,
                    name: frame.name.clone(),
                    place: frame
                        .path
                        .as_ref()
                        .and_then(|path| path.file_name())
                        .map(|name| format!("{}:{}", name.to_string_lossy(), frame.line + 1))
                        .unwrap_or_default(),
                })
                .collect(),
            frame: view.frame,
            scopes,
            watches: view
                .watched
                .iter()
                .map(|watched| {
                    (
                        watched.expression.clone(),
                        match &watched.value {
                            Ok(variable) => variable.value.clone(),
                            Err(reason) => reason.clone(),
                        },
                    )
                })
                .collect(),
            console: view
                .lines
                .iter()
                .rev()
                .take(CONSOLE_KEPT)
                .rev()
                .map(|line| line.text.clone())
                .collect(),
            inline_values,
        }
    }

    /// Appends the variables behind `reference` in `view`, each followed by its members when expanded, at `depth`.
    fn flatten(
        &self,
        view: &DebugView,
        reference: i64,
        depth: usize,
        out: &mut Vec<DebugVariableEntry>,
    ) {
        let Some(variables) = view.variables_of(reference) else {
            return;
        };
        for variable in variables {
            let expanded = variable.reference != 0 && self.expanded.contains(&variable.reference);
            out.push(DebugVariableEntry {
                name: variable.name.clone(),
                value: variable.value.clone(),
                kind: variable.kind.clone(),
                reference: variable.reference,
                expanded,
                depth,
            });
            if expanded && depth < 8 {
                self.flatten(view, variable.reference, depth + 1, out);
            }
        }
    }
}

/// What a session labelled `label` looks like before the server told more.
fn empty_view(label: String) -> DebugView {
    DebugView {
        label,
        standing: Standing::Starting,
        reason: None,
        threads: Vec::new(),
        thread: None,
        frames: Vec::new(),
        frame: None,
        scopes: Vec::new(),
        variables: Vec::new(),
        watches: Vec::new(),
        watched: Vec::new(),
        lines: Vec::new(),
        line_count: 0,
        placed: Vec::new(),
    }
}

/// A plain breakpoint on `line`.
fn breakpoint(line: usize) -> Breakpoint {
    Breakpoint {
        line,
        ..Breakpoint::default()
    }
}

/// Whether the adapter of `view` placed the breakpoint on `line` of `path`, when it said.
fn verified(view: &DebugView, path: &str, line: usize) -> Option<bool> {
    view.placed
        .iter()
        .find(|(held, _)| held.ends_with(path))
        .and_then(|(_, placed)| placed.iter().find(|placed| placed.line == line))
        .map(|placed| placed.verified)
}

/// What the tab of `shell`, at `index` among its folder's, reads.
fn shell_title(shell: &Shell, index: usize) -> String {
    if let Some(task) = &shell.task {
        return task.clone();
    }
    match shell.emulator.title().map(str::trim) {
        Some(title) if !title.is_empty() => title.to_owned(),
        _ => format!("Terminal {}", index + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shells of the start page follow the session started from it.
    #[test]
    fn shells_are_handed_over() {
        let mut dock = Dock::default();
        dock.follow("/repo".to_owned());
        let key = dock.add_shell(None);
        dock.hand_over("/worktree".to_owned());
        dock.follow("/worktree".to_owned());
        assert_eq!(dock.place_shells().count(), 1);
        assert_eq!(dock.shown.get("/worktree"), Some(&key));
    }

    /// A connected dock asks for a shell in the place, and a second one becomes a second tab that shows.
    #[test]
    fn new_shells_become_tabs() {
        let mut dock = Dock::default();
        dock.connected();
        dock.new_shell();
        let mut workspace = Workspace::default();
        dock.publish(&mut workspace);
        assert_eq!(workspace.terminals.len(), 2);
        assert_eq!(workspace.terminal_shown, Some(workspace.terminals[1].key));
        dock.close(workspace.terminals[1].key);
        dock.publish(&mut workspace);
        assert_eq!(workspace.terminals.len(), 1);
        assert_eq!(workspace.terminal_shown, Some(workspace.terminals[0].key));
    }

    /// Diagnostics fill the problems, worst first, and an empty report clears a file's.
    #[test]
    fn diagnostics_fill_problems() {
        let mut dock = Dock::default();
        let diagnostic = |severity, message: &str| pm_protocol::Diagnostic {
            span: Default::default(),
            severity,
            message: message.to_owned(),
            source: None,
        };
        dock.diagnostics(
            "/w".to_owned(),
            "a.rs".to_owned(),
            vec![
                diagnostic(pm_protocol::Severity::Warning, "unused"),
                diagnostic(pm_protocol::Severity::Error, "broken"),
            ],
        );
        let mut workspace = Workspace::default();
        dock.publish(&mut workspace);
        assert_eq!(workspace.problems[0].message, "broken");
        dock.diagnostics("/w".to_owned(), "a.rs".to_owned(), Vec::new());
        dock.publish(&mut workspace);
        assert!(workspace.problems.is_empty());
    }

    /// Toggling a breakpoint twice takes it away again.
    #[test]
    fn breakpoints_toggle() {
        let mut dock = Dock::default();
        dock.toggle_breakpoint("src/main.rs".to_owned(), 4);
        assert_eq!(dock.debug_files()[0].breakpoints[0].line, 4);
        dock.toggle_breakpoint("src/main.rs".to_owned(), 4);
        assert!(dock.debug_files().is_empty());
    }
}
