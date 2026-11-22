//! Debugging as the workspace holds it: the breakpoints set in the gutter, the program being debugged, and what the reader does to it.

use crate::areas::DebugEvent as DebugControl;
use crate::target::Target;
use std::collections::{BTreeMap, BTreeSet};

use pm_protocol::{
    Breakpoint, DebugAction, DebugAdapter, DebugEvent, DebugFile, DebugProcess, DebugScenario,
    DebugView, Location, Position, Span, Standing,
};
use rustle::TextFieldState;

use crate::listing::{Items, Listing};
use crate::quick_choices::Choosing;
use crate::state::{Outgoing, WorkspaceState};

/// How many rows each list of the debug tab shows at most.
pub const DEBUG_ROWS: usize = 1000;

/// How a breakpoint is marked in the gutter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dot {
    /// The program stops here.
    Plain,
    /// The program stops here when a condition holds, or logs instead.
    Conditional,
    /// The adapter could not put it here.
    Unplaced,
}

/// What the gutter of a file shows about debugging.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gutter {
    /// The breakpoints, by line counted from zero.
    pub marks: Vec<(usize, Dot)>,
    /// The line the program is paused on.
    pub paused: Option<usize>,
    /// The lines where a place that can be run starts.
    pub runs: Vec<usize>,
}

impl Gutter {
    /// How the breakpoint on `line` is marked, when there is one.
    pub fn dot(&self, line: usize) -> Option<Dot> {
        self.marks
            .iter()
            .find(|(held, _)| *held == line)
            .map(|(_, dot)| *dot)
    }
}

/// A program being debugged in the folder of a session.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// The session whose folder it is debugged in, by position.
    pub session: usize,
    /// What the server knows it by.
    pub debug: u64,
    /// What it looks like now.
    pub view: DebugView,
}

/// What the workspace holds of debugging.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebugState {
    /// The breakpoints of each file of each session, by position and path from the root of its folder.
    pub breakpoints: BTreeMap<(usize, String), Vec<Breakpoint>>,
    /// The program being debugged, or the last one while the reader has not started another.
    pub run: Option<Run>,
    /// The session a start is waiting on the server for.
    pub starting: Option<usize>,
    /// What the folder can be debugged as, as the server last listed it.
    pub scenarios: Vec<DebugScenario>,
    /// The file in front when the scenarios were asked for.
    pub scenarios_for: Option<String>,
    /// The programs a debugger can attach to, and the adapters that can.
    pub processes: (Vec<DebugAdapter>, Vec<DebugProcess>),
    /// The variables unfolded, by the reference the adapter gave them.
    pub unfolded: BTreeSet<i64>,
    /// The expressions watched.
    pub watches: Vec<String>,
    /// The expression being typed to be evaluated.
    pub console: TextFieldState,
    /// The expression being typed to be watched.
    pub watch: TextFieldState,
    /// How far the stack is scrolled down.
    pub stack_scroll: rustle::ScrollState,
    /// How far the variables are scrolled down.
    pub variables_scroll: rustle::ScrollState,
    /// How far the console is scrolled back from its end.
    pub console_back: rustle::ScrollState,
    /// Why the last start failed.
    pub trouble: Option<String>,
    /// The frame the reader chose in the stack, whose line is to be shown.
    pub reveal: Option<(usize, pm_protocol::DebugFrame)>,
    /// Which names of the file the program is paused in the language server said to show the values of, and what they were asked for.
    pub inline: Option<crate::inline_values::InlineAsked>,
}

/// One row of the variables: a scope of the frame or a variable, at its depth.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableRow {
    /// How deep it is among its parents.
    pub depth: usize,
    /// What it is called.
    pub name: String,
    /// What it holds, or nothing for a scope.
    pub value: String,
    /// Its type, when the adapter says.
    pub kind: Option<String>,
    /// What its members are asked for by, or zero when it has none.
    pub reference: i64,
    /// Whether its members are showing.
    pub unfolded: bool,
}

/// How deep the variables are followed.
const DEEPEST: usize = 6;

/// The rows of `view`'s scopes and, under each variable `unfolded`, its members.
pub fn variable_rows(view: &DebugView, unfolded: &BTreeSet<i64>) -> Vec<VariableRow> {
    fn members(
        view: &DebugView,
        unfolded: &BTreeSet<i64>,
        reference: i64,
        depth: usize,
        rows: &mut Vec<VariableRow>,
    ) {
        if depth > DEEPEST {
            return;
        }
        for variable in view.variables_of(reference).unwrap_or_default() {
            let open = variable.reference != 0 && unfolded.contains(&variable.reference);
            rows.push(VariableRow {
                depth,
                name: variable.name.clone(),
                value: variable.value.clone(),
                kind: variable.kind.clone(),
                reference: variable.reference,
                unfolded: open,
            });
            if open {
                members(view, unfolded, variable.reference, depth + 1, rows);
            }
        }
    }
    let mut rows = Vec::new();
    for scope in &view.scopes {
        let open = !scope.expensive || unfolded.contains(&scope.reference);
        rows.push(VariableRow {
            depth: 0,
            name: scope.name.clone(),
            value: String::new(),
            kind: None,
            reference: scope.reference,
            unfolded: open,
        });
        if open {
            members(view, unfolded, scope.reference, 1, &mut rows);
        }
    }
    rows
}

impl DebugState {
    /// The program being debugged, while it has not ended.
    pub fn live(&self) -> Option<&Run> {
        self.run
            .as_ref()
            .filter(|run| run.view.standing != Standing::Ended)
    }

    /// Whether a program is being debugged, or about to be.
    pub fn busy(&self) -> bool {
        self.starting.is_some() || self.live().is_some()
    }

    /// The breakpoints of the file at `path` of the session at `session`.
    pub fn breakpoints_in(&self, session: usize, path: &str) -> &[Breakpoint] {
        self.breakpoints
            .get(&(session, path.to_owned()))
            .map_or(&[], Vec::as_slice)
    }

    /// The line the program is paused on in the file at `path` of the session at `session`, counted from zero, given the session's folder `root`.
    pub fn paused_on(&self, session: usize, root: &str, path: &str) -> Option<usize> {
        let run = self.run.as_ref().filter(|run| run.session == session)?;
        if run.view.standing != Standing::Stopped {
            return None;
        }
        let frame = run.view.selected_frame()?;
        let held = frame.path.as_ref()?.to_string_lossy();
        let relative = held.strip_prefix(root)?.trim_start_matches(['/', '\\']);
        (relative == path).then_some(frame.line)
    }

    /// The breakpoints of every file of the session at `session`, for a program about to start.
    pub fn files_of(&self, session: usize) -> Vec<DebugFile> {
        self.breakpoints
            .iter()
            .filter(|((held, _), breakpoints)| *held == session && !breakpoints.is_empty())
            .map(|((_, path), breakpoints)| DebugFile {
                path: path.clone(),
                breakpoints: breakpoints.clone(),
            })
            .collect()
    }
}

impl WorkspaceState {
    /// The folder of the session at `session`, as the server knows it.
    pub(crate) fn debug_root(&self, session: usize) -> Option<String> {
        let held = self.sessions.get(session)?;
        if held.directory.is_empty() {
            let project = self.project_of(held)?;
            return Some(project.directory().to_string_lossy().into_owned());
        }
        Some(held.directory.clone())
    }

    /// The path of the file at `path`, from the root of the folder of the session at `session`, when it is inside it.
    pub(crate) fn debug_relative(&self, session: usize, path: &std::path::Path) -> Option<String> {
        let root = self.debug_root(session)?;
        let path = path.to_string_lossy();
        path.strip_prefix(root.trim_end_matches(['/', '\\']))
            .map(|rest| rest.trim_start_matches(['/', '\\']).to_owned())
            .filter(|relative| !relative.is_empty() && relative.len() < path.len())
    }

    /// The path of the file shown, from the root of the session's folder, when it is the folder's own and not the last commit's.
    fn debug_file(&self) -> Option<String> {
        self.shown_viewer()
            .filter(|viewer| !viewer.head)
            .map(|viewer| viewer.path.clone())
    }

    /// What the gutter of the open file at `file` of the session being worked in shows: its breakpoints, and the line the program is paused on.
    pub fn gutter_of(&self, file: usize) -> Gutter {
        let (Some(session), Some(viewer)) = (self.review_index(), self.git.viewers.get(file))
        else {
            return Gutter::default();
        };
        if viewer.head {
            return Gutter::default();
        }
        let root = self.debug_root(session);
        let paused = root
            .as_deref()
            .and_then(|root| self.debug.paused_on(session, root, &viewer.path));
        let unplaced: Vec<usize> = match (&self.debug.run, &root) {
            (Some(run), Some(root)) if run.session == session => {
                let path = std::path::Path::new(root).join(&viewer.path);
                run.view
                    .placed_in(&path)
                    .iter()
                    .filter(|placed| !placed.verified)
                    .map(|placed| placed.line)
                    .collect()
            }
            _ => Vec::new(),
        };
        let marks = self
            .debug
            .breakpoints_in(session, &viewer.path)
            .iter()
            .map(|breakpoint| {
                let how = if unplaced.contains(&breakpoint.line) {
                    Dot::Unplaced
                } else if breakpoint.condition.is_some()
                    || breakpoint.hits.is_some()
                    || breakpoint.log.is_some()
                {
                    Dot::Conditional
                } else {
                    Dot::Plain
                };
                (breakpoint.line, how)
            })
            .collect();
        Gutter {
            marks,
            paused,
            runs: self.run_lines_of(file),
        }
    }

    /// Sets or clears the breakpoint on `line` of the open file at `file`.
    pub(crate) fn toggle_breakpoint(&mut self, file: usize, line: usize) {
        let Some(session) = self.review_index() else {
            return;
        };
        let Some(path) = self
            .git
            .viewers
            .get(file)
            .filter(|viewer| !viewer.head)
            .map(|viewer| viewer.path.clone())
        else {
            return;
        };
        let held = self
            .debug
            .breakpoints
            .entry((session, path.clone()))
            .or_default();
        match held.iter().position(|breakpoint| breakpoint.line == line) {
            Some(place) => {
                held.remove(place);
            }
            None => {
                held.push(Breakpoint {
                    line,
                    ..Breakpoint::default()
                });
                held.sort_by_key(|breakpoint| breakpoint.line);
            }
        }
        self.send_breakpoints(session, &path);
    }

    /// Sets or clears the breakpoint on the line the caret of the file shown is on.
    pub fn breakpoint_toggled_at_caret(mut self) -> Self {
        let Some(file) = self.git.shown else {
            return self;
        };
        let Some(line) = self
            .shown_document()
            .map(|document| document.line_of(document.field.caret()))
        else {
            return self;
        };
        self.toggle_breakpoint(file, line);
        self
    }

    /// Tells the program being debugged in the session at `session` the breakpoints of `path` now.
    fn send_breakpoints(&mut self, session: usize, path: &str) {
        let Some(run) = self.debug.live().filter(|run| run.session == session) else {
            return;
        };
        let debug = run.debug;
        let breakpoints = self.debug.breakpoints_in(session, path).to_vec();
        self.outbox.push(Outgoing::DebugDo {
            session,
            debug,
            action: DebugAction::SetBreakpoints {
                path: path.to_owned(),
                breakpoints,
            },
        });
    }

    /// Starts debugging: lists what the folder of the session being worked in can be debugged as at `at`, to pick one, or runs the program on when one is paused.
    pub fn debug_started(mut self, at: (f32, f32)) -> Self {
        if self.debug.live().is_some() {
            self.send_debug(DebugAction::Continue);
            return self;
        }
        let Some(session) = self.review_index() else {
            return self;
        };
        let file = self.debug_file();
        self.outbox.push(Outgoing::DebugScenarios {
            session,
            file: file.clone(),
        });
        self.debug.scenarios_for = file;
        self.debug.trouble = None;
        self.await_choices(session, Choosing::DebugScenario, at);
        self
    }

    /// Lists the programs running on the machine of the session being worked in at `at`, to attach to one.
    pub fn debug_attach_requested(mut self, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        self.outbox.push(Outgoing::DebugProcesses { session });
        self.await_choices(session, Choosing::DebugProcess, at);
        self
    }

    /// Opens the quick list of `choosing` for the session at `session` at `at`, empty until the server answers.
    fn await_choices(&mut self, session: usize, choosing: Choosing, at: (f32, f32)) {
        let items = Items::Choices {
            choosing,
            choices: Vec::new(),
            filter: TextFieldState::new(""),
            matched: Vec::new(),
        };
        self.listing = Some(Listing {
            quick: true,
            ..Listing::waiting(session, items, at)
        });
    }

    /// Fills the quick list of `choosing` anew now that the server answered for the session at `session`.
    fn choices_answered(&mut self, session: usize, choosing: Choosing) {
        self.choices_renewed(choosing);
        if let Some(listing) = self.listing.as_mut()
            && listing.session == session
        {
            listing.answered = true;
        }
    }

    /// The state after the server listed what the folder of the session at `session` can be debugged as.
    pub fn with_debug_scenarios(mut self, session: usize, scenarios: Vec<DebugScenario>) -> Self {
        self.debug.scenarios = scenarios;
        self.choices_answered(session, Choosing::DebugScenario);
        self
    }

    /// The state after the server listed the programs that can be attached to and the adapters that can.
    pub fn with_debug_processes(
        mut self,
        session: usize,
        adapters: Vec<DebugAdapter>,
        processes: Vec<DebugProcess>,
    ) -> Self {
        self.debug.processes = (adapters, processes);
        self.choices_answered(session, Choosing::DebugProcess);
        self
    }

    /// Starts debugging the folder of the session being worked in as the scenario at `index` of those listed.
    pub(crate) fn debug_scenario_taken(&mut self, index: usize) {
        let Some(session) = self.review_index() else {
            return;
        };
        self.debug.starting = Some(session);
        self.debug.trouble = None;
        self.outbox.push(Outgoing::DebugStart {
            session,
            file: self.debug.scenarios_for.clone(),
            scenario: index,
            files: self.debug.files_of(session),
        });
    }

    /// Attaches to the process `pid`, with the adapter that suits what it is called.
    pub(crate) fn debug_process_taken(&mut self, pid: u32) {
        let Some(session) = self.review_index() else {
            return;
        };
        let (adapters, processes) = &self.debug.processes;
        let name = processes
            .iter()
            .find(|process| process.pid == pid)
            .map(|process| process.name.to_lowercase())
            .unwrap_or_default();
        let Some(adapter) = adapters
            .iter()
            .filter_map(|adapter| {
                adapter
                    .attach_names
                    .iter()
                    .filter(|start| name.starts_with(start.as_str()))
                    .map(String::len)
                    .max()
                    .map(|fit| (fit, adapter))
            })
            .max_by_key(|(fit, _)| *fit)
            .map(|(_, adapter)| adapter)
            .or_else(|| adapters.first())
            .map(|adapter| adapter.id.clone())
        else {
            self.debug.trouble = Some("No debug adapter is installed to attach with".to_owned());
            return;
        };
        self.debug.starting = Some(session);
        self.debug.trouble = None;
        self.outbox.push(Outgoing::DebugAttach {
            session,
            adapter,
            pid,
            files: self.debug.files_of(session),
        });
    }

    /// The state after the server started debugging for the session at `session`, or said it could not.
    pub fn with_debug_started(
        mut self,
        session: usize,
        started: Result<(u64, DebugView), String>,
    ) -> Self {
        self.debug.starting = None;
        match started {
            Ok((debug, view)) => {
                self.debug.unfolded.clear();
                self.debug.console_back.jump_to(0.0);
                self.debug.run = Some(Run {
                    session,
                    debug,
                    view,
                });
                self.bottom = crate::dock::BottomTab::Debug;
                self.terminal = true;
                if !self.debug.watches.is_empty() {
                    self.outbox.push(Outgoing::DebugDo {
                        session,
                        debug,
                        action: DebugAction::SetWatches {
                            watches: self.debug.watches.clone(),
                        },
                    });
                }
            }
            Err(reason) => {
                self.debug.trouble = Some(reason.clone());
                self = self.noted(session, &reason);
            }
        }
        self
    }

    /// The state after the server said the program numbered `debug` moved on, with what happened since.
    pub fn with_debug(mut self, debug: u64, view: DebugView, events: Vec<DebugEvent>) -> Self {
        let Some(run) = self.debug.run.as_mut().filter(|run| run.debug == debug) else {
            return self;
        };
        let session = run.session;
        run.view = view;
        for event in events {
            match event {
                DebugEvent::Paused(frame) => self = self.debug_paused(session, frame),
                DebugEvent::StartRefused(reason) => {
                    self.debug.trouble = Some(reason.clone());
                    self = self.noted(session, &format!("Could not debug: {reason}"));
                }
                DebugEvent::Ended => {}
            }
        }
        self
    }

    /// Shows the line a program paused on, in the session's folder, when the file is in it.
    fn debug_paused(self, session: usize, frame: pm_protocol::DebugFrame) -> Self {
        let Some(relative) = frame
            .path
            .as_deref()
            .and_then(|path| self.debug_relative(session, path))
        else {
            return self;
        };
        let place = Position::new(frame.line as u32, 0);
        self.go_to_location(
            session,
            Location {
                path: relative,
                span: Span::new(place, place),
                inside: true,
                ..Location::default()
            },
        )
    }

    /// The state after the server no longer knows the program numbered `debug`, as when it was restarted.
    pub fn with_debug_lost(mut self, debug: u64) -> Self {
        if let Some(run) = self.debug.run.as_mut().filter(|run| run.debug == debug) {
            run.view.standing = Standing::Ended;
        }
        self
    }

    /// Does `action` to the program being debugged, when there is one.
    pub fn debug_do(mut self, action: DebugAction) -> Self {
        self.send_debug(action);
        self
    }

    /// Queues `action` for the program being debugged, when there is one.
    fn send_debug(&mut self, action: DebugAction) {
        if let Some(run) = self.debug.live() {
            let (session, debug) = (run.session, run.debug);
            self.outbox.push(Outgoing::DebugDo {
                session,
                debug,
                action,
            });
        }
    }

    /// The state showing the line of the frame the reader chose in the stack, when they chose one.
    pub(crate) fn debug_revealed(mut self) -> Self {
        match self.debug.reveal.take() {
            Some((session, frame)) => self.debug_paused(session, frame),
            None => self,
        }
    }

    /// Carries out a press on one of the debug tab's controls, and says whether `id` was one.
    pub(crate) fn press_debug(&mut self, id: Target) -> bool {
        let Target::Debug(control) = id else {
            return false;
        };
        let action = match control {
            DebugControl::DebugContinue => DebugAction::Continue,
            DebugControl::DebugPause => DebugAction::Pause,
            DebugControl::DebugStepOver => DebugAction::StepOver,
            DebugControl::DebugStepIn => DebugAction::StepIn,
            DebugControl::DebugStepOut => DebugAction::StepOut,
            DebugControl::DebugStop => DebugAction::Stop,
            DebugControl::DebugTab => {
                self.bottom = crate::dock::BottomTab::Debug;
                return true;
            }
            DebugControl::DebugConsoleSubmit => {
                self.evaluate_typed();
                return true;
            }
            DebugControl::DebugWatchAdd => {
                self.watch_typed();
                return true;
            }
            DebugControl::DebugConsoleField | DebugControl::DebugWatchField => return true,
            _ => return self.press_debug_row(control),
        };
        self.send_debug(action);
        true
    }

    /// Carries out a press on a row of the stack, the variables or the watches, and says whether `control` was one.
    fn press_debug_row(&mut self, control: DebugControl) -> bool {
        match control {
            DebugControl::DebugFrame(index) if index < DEBUG_ROWS => {
                let frame = self
                    .debug
                    .live()
                    .and_then(|run| run.view.frames.get(index).map(|frame| frame.id));
                if let Some(frame) = frame {
                    self.debug.reveal = self.debug.run.as_mut().map(|run| {
                        run.view.frame = Some(frame);
                        (run.session, run.view.frames[index].clone())
                    });
                    self.send_debug(DebugAction::SelectFrame { frame });
                }
            }
            DebugControl::DebugVariable(index) if index < DEBUG_ROWS => self.toggle_variable(index),
            DebugControl::DebugWatchRemove(index) if index < DEBUG_ROWS => {
                if index < self.debug.watches.len() {
                    self.debug.watches.remove(index);
                    self.send_watches();
                }
            }
            _ => return false,
        }
        true
    }

    /// Unfolds the variable at row `index`, asking for its members, or folds it.
    fn toggle_variable(&mut self, index: usize) {
        let Some(run) = self.debug.live() else {
            return;
        };
        let rows = variable_rows(&run.view, &self.debug.unfolded);
        let Some(row) = rows.get(index).filter(|row| row.reference != 0) else {
            return;
        };
        let reference = row.reference;
        let asked = run.view.variables_of(reference).is_some();
        if !self.debug.unfolded.remove(&reference) {
            self.debug.unfolded.insert(reference);
            if !asked {
                self.send_debug(DebugAction::Expand { reference });
            }
        }
    }

    /// Asks the program what the expression typed in the console comes to.
    fn evaluate_typed(&mut self) {
        let expression = self.debug.console.text().trim().to_owned();
        if expression.is_empty() {
            return;
        }
        self.debug.console = TextFieldState::new("");
        self.debug.console_back.jump_to(0.0);
        self.send_debug(DebugAction::Evaluate { expression });
    }

    /// Keeps the expression typed in the watch field and has the program evaluate it.
    fn watch_typed(&mut self) {
        let expression = self.debug.watch.text().trim().to_owned();
        if expression.is_empty() {
            return;
        }
        self.debug.watch = TextFieldState::new("");
        if !self.debug.watches.contains(&expression) {
            self.debug.watches.push(expression);
        }
        self.send_watches();
    }

    /// Tells the program being debugged which expressions are watched.
    fn send_watches(&mut self) {
        let watches = self.debug.watches.clone();
        self.send_debug(DebugAction::SetWatches { watches });
    }

    /// Brings the breakpoints and the program being debugged up to date after the sessions moved: `moved` says, for each old position, the new one, or nothing for a session that is gone.
    pub(crate) fn remap_debug(&mut self, moved: &[Option<usize>]) {
        self.debug.breakpoints = std::mem::take(&mut self.debug.breakpoints)
            .into_iter()
            .filter_map(|((session, path), breakpoints)| {
                Some(((moved.get(session).copied().flatten()?, path), breakpoints))
            })
            .collect();
        self.debug.run = self.debug.run.take().and_then(|mut run| {
            run.session = moved.get(run.session).copied().flatten()?;
            Some(run)
        });
        self.debug.starting = self
            .debug
            .starting
            .and_then(|session| moved.get(session).copied().flatten());
    }
}
