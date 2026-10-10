//! What the window does about debugging: breakpoints, and the program they
//! stop.
//!
//! A breakpoint is set in the gutter of a file and belongs to the worktree
//! the file was opened from; a program is debugged in the worktree the window
//! is pointed at, and read in the bottom panel's debug view. Which file a
//! paused program is brought up in and who has the keyboard meanwhile are
//! the window's, and they are here. Every debugging command goes through [`App::debug_action`]
//! or [`App::debug_command`].

use pm_core::Scope;
use pm_dap::{Breakpoint as DapBreakpoint, Event, Scenario, Standing};
use pm_text::Position;

use crate::app::places::Place;
use crate::app::{App, Writing};
use crate::editor::{Breakpoint, FileId, Mark};
use crate::keymap::Action;
use crate::message::Message;
use crate::panel::PanelView;
use crate::panes::PaneId;
use crate::picker::{Choice, Kind, Row};
use crate::tasks::Shown;

impl App {
    /// Carries out the debugging commands a keybinding or the palette names.
    ///
    /// The answer says whether `action` was one of them, so that the window
    /// can go on trying the rest.
    pub(super) fn debug_action(&mut self, action: Action) -> bool {
        match action {
            Action::DebugStart => self.open_picker(Kind::Debug),
            Action::DebugAttach => self.open_picker(Kind::Processes),
            Action::DebugEditCondition => self.edit_cursor_breakpoint(Kind::BreakpointCondition),
            Action::DebugEditHits => self.edit_cursor_breakpoint(Kind::BreakpointHits),
            Action::DebugEditLog => self.edit_cursor_breakpoint(Kind::BreakpointLog),
            Action::DebugAddWatch => self.open_watch_prompt(None),
            Action::DebugContinue => self.continue_or_start(),
            Action::DebugPause => self.with_session(pm_dap::Session::pause),
            Action::DebugStepOver => self.with_session(pm_dap::Session::step_over),
            Action::DebugStepInto => self.with_session(pm_dap::Session::step_in),
            Action::DebugStepOut => self.with_session(pm_dap::Session::step_out),
            Action::DebugStop => {
                if let Some(scope) = self.scope() {
                    self.debuggers.stop(scope);
                }
            }
            Action::DebugRestart => self.restart_debugging(),
            Action::ToggleBreakpoint => {
                if let Some(line) = self.with_buffer(|buffer| buffer.selection().head.line) {
                    self.toggle_breakpoint(self.panes.focus(), line);
                }
            }
            Action::ClearBreakpoints => {
                if let Some(scope) = self.scope() {
                    self.debuggers.clear(scope);
                }
            }
            Action::OpenDebugger => {
                if let Some(scope) = self.scope() {
                    self.show_debugger(scope);
                }
            }
            _ => return false,
        }
        true
    }

    /// Carries out the messages the gutter and the debugger's view send.
    pub(super) fn debug_command(&mut self, message: Message) -> bool {
        match message {
            Message::ToggleBreakpoint(pane, at) => {
                self.focus_pane(pane);
                self.toggle_breakpoint(pane, at.line);
            }
            Message::ShowBreakpointMenu(pane, at) => {
                self.focus_pane(pane);
                self.open_menu(crate::workspace::MenuTarget::Breakpoint(pane, at.line));
            }
            Message::EditBreakpoint(pane, line, kind) => self.edit_breakpoint_at(pane, line, kind),
            Message::RemoveWatch(index) => {
                if let Some(scope) = self.scope() {
                    let mut watches = self.debuggers.watches(scope).to_vec();
                    if index < watches.len() {
                        watches.remove(index);
                        self.debuggers.set_watches(scope, watches);
                    }
                }
            }
            Message::EditWatch(index) => {
                if self.watch_clicks.press(index) == 2 {
                    self.open_watch_prompt(Some(index));
                }
            }
            Message::ToggleWatchSection => {
                if let Some(debugger) = self.scope().and_then(|scope| self.debuggers.get_mut(scope))
                {
                    debugger.toggle_scope("Watch");
                }
            }
            Message::ActOnDebugger(action) => {
                self.debug_action(action);
            }
            Message::SelectFrame(frame) => self.select_frame(frame),
            Message::ToggleVariable(reference) => {
                if let Some(debugger) = self.scope().and_then(|scope| self.debuggers.get_mut(scope))
                {
                    debugger.toggle(reference);
                }
            }
            Message::ToggleDebugScope(place) => {
                let Some(scope) = self.scope() else {
                    return true;
                };
                let name = self
                    .debuggers
                    .get(scope)
                    .and_then(|debugger| debugger.session().scopes().into_iter().nth(place))
                    .map(|found| found.name);
                if let (Some(name), Some(debugger)) = (name, self.debuggers.get_mut(scope)) {
                    debugger.toggle_scope(&name);
                }
            }
            Message::WriteDebugConsole(phase, anchor, head) => {
                if let Some(scope) = self.scope() {
                    self.point_in(Writing::Console(scope), phase, anchor, head);
                }
            }
            _ => return false,
        }
        true
    }

    /// What the worktree in front can be debugged as, as the picker offers
    /// it.
    ///
    /// A scenario whose adapter the editor does not have, or has and cannot
    /// find, is listed unpickable with the reason in its place: a list that
    /// left it out would leave the reader wondering where their
    /// configuration went.
    pub(super) fn debug_rows(&self) -> Vec<Row> {
        let Some((scope, root)) = self
            .scope()
            .and_then(|scope| Some((scope, self.root_of(scope)?)))
        else {
            return Vec::new();
        };
        let file = self
            .active_file_id()
            .and_then(|file| self.editor.path(file));
        pm_dap::scenarios(&root, file.as_deref())
            .into_iter()
            .map(|scenario| {
                let (detail, enabled) = match scenario.adapter {
                    None => (format!("no debug adapter for `{}`", scenario.kind), false),
                    Some(adapter) if adapter.program_on(&root.host).is_none() => {
                        (format!("{} is not installed", adapter.name), false)
                    }
                    Some(adapter) => {
                        let from = scenario.source.as_ref().map_or_else(
                            || "current file".to_owned(),
                            |source| source.display().to_string(),
                        );
                        (format!("{} · {from}", adapter.name), true)
                    }
                };
                Row {
                    section: None,
                    label: scenario.label.clone(),
                    detail,
                    choice: Choice::Debug(scope, Box::new(scenario)),
                    enabled,
                }
            })
            .collect()
    }

    /// Lists processes the editor can attach to.
    pub(super) fn process_rows(&self) -> Vec<Row> {
        let Some(root) = self.scope().and_then(|scope| self.root_of(scope)) else {
            return Vec::new();
        };
        pm_dap::processes_on(&root.host)
            .into_iter()
            .map(|process| Row {
                section: None,
                label: process.name,
                detail: format!("{} · {}", process.pid, process.command),
                choice: Choice::Process(process.pid),
                enabled: true,
            })
            .collect()
    }

    /// Lists installed adapters able to attach to the chosen process.
    pub(super) fn attach_adapter_rows(&self) -> Vec<Row> {
        let Some(pid) = self.attach_pid else {
            return Vec::new();
        };
        let Some(scope) = self.scope() else {
            return Vec::new();
        };
        let Some(root) = self.root_of(scope) else {
            return Vec::new();
        };
        let python = pm_dap::processes_on(&root.host)
            .iter()
            .find(|process| process.pid == pid)
            .is_some_and(|process| process.name.starts_with("python"));
        let adapters = if python { [2, 0, 1, 3] } else { [0, 1, 3, 2] };
        adapters
            .into_iter()
            .filter_map(|index| {
                let adapter = pm_dap::ADAPTERS[index];
                adapter.program_on(&root.host).is_some().then(|| Row {
                    section: None,
                    label: adapter.name.to_owned(),
                    detail: format!("Attach to {pid}"),
                    choice: Choice::Debug(scope, Box::new(adapter.attach(pid))),
                    enabled: true,
                })
            })
            .collect()
    }

    /// Opens the adapter picker for one process.
    pub(super) fn choose_attach_process(&mut self, pid: u32) {
        self.attach_pid = Some(pid);
        self.open_picker(Kind::AttachAdapters);
    }

    /// Starts debugging `scenario` in `scope`, and shows the debugger.
    pub(super) fn start_debugging(&mut self, scope: Scope, scenario: Scenario) {
        if let Some(label) = &scenario.before {
            let Some(task) = self
                .available_tasks(scope)
                .into_iter()
                .find(|task| task.label == *label)
            else {
                self.notices
                    .trouble(format!("No task called `{label}`"), None);
                return;
            };
            if let Some(run) = self.run_task(scope, &task, Shown::Front) {
                self.pending_debug.insert(run, (scope, scenario));
            }
            return;
        }
        self.start_debug_adapter(scope, scenario);
    }

    /// Starts the adapter after any task required by the scenario succeeded.
    pub(super) fn start_debug_adapter(&mut self, scope: Scope, scenario: Scenario) {
        let Some(root) = self.root_of(scope) else {
            return;
        };
        if let Err(trouble) = self.debuggers.start(scope, &root, scenario) {
            return self.say_trouble("Debugging could not start", &trouble);
        }
        self.show_debugger(scope);
    }

    /// Takes in what the debug adapters have said, answering whether there
    /// is anything new to draw.
    ///
    /// A program that pauses is brought up where it paused, in the pane in
    /// front, so the reader is looking at the line as soon as it is the line
    /// that matters.
    pub(super) fn take_debugged(&mut self) -> bool {
        let (changed, events) = self.debuggers.pump();
        for (scope, event) in events {
            match event {
                Event::Paused(frame) if self.scope() == Some(scope) => {
                    self.bring_up(scope, &frame);
                }
                Event::StartRefused(reason) => {
                    self.say_trouble("Debugging could not start", &reason)
                }
                Event::Paused(_) | Event::Ended => {}
            }
        }
        changed
    }

    /// Scrolls the debugger's list under the pointer by `delta` logical
    /// pixels, answering whether there was one.
    pub(super) fn scroll_debugger(&mut self, delta: f32) -> bool {
        let (Some(pointer), Some(scope)) = (self.pointer, self.scope()) else {
            return false;
        };
        self.showing_debugger()
            && self
                .debuggers
                .get_mut(scope)
                .is_some_and(|debugger| debugger.scroll(pointer, delta))
    }

    /// The breakpoints of `file`, as its gutter marks them.
    ///
    /// A breakpoint the program being debugged could not place is marked as
    /// such; one it has not been asked about yet, or placed where it was
    /// set, is marked as set.
    pub(super) fn breakpoints_of(&self, file: FileId) -> Vec<Breakpoint> {
        let (Some(scope), Some(path)) = (self.editor.scope_of(file), self.editor.path(file)) else {
            return Vec::new();
        };
        let placed = self
            .debuggers
            .live(scope)
            .map(|session| session.placed(&path))
            .unwrap_or_default();
        self.debuggers
            .breakpoints(scope, &path)
            .into_iter()
            .map(|breakpoint| Breakpoint {
                line: breakpoint.line,
                kind: if breakpoint
                    .log
                    .as_ref()
                    .is_some_and(|value| !value.is_empty())
                {
                    Mark::Log
                } else if breakpoint
                    .condition
                    .as_ref()
                    .is_some_and(|value| !value.is_empty())
                    || breakpoint
                        .hits
                        .as_ref()
                        .is_some_and(|value| !value.is_empty())
                {
                    Mark::Conditional
                } else {
                    Mark::Plain
                },
                message: placed
                    .iter()
                    .find(|mark| mark.line == breakpoint.line)
                    .and_then(|mark| mark.message.clone()),
                placed: placed
                    .iter()
                    .find(|mark| mark.line == breakpoint.line)
                    .is_none_or(|mark| mark.verified),
            })
            .collect()
    }

    /// The line of `file` the paused program is looking at, if it is in it.
    pub(super) fn stopped_in(&self, file: FileId) -> Option<usize> {
        let scope = self.editor.scope_of(file)?;
        let path = self.editor.path(file)?;
        let session = self.debuggers.live(scope)?;
        if session.standing() != Standing::Stopped {
            return None;
        }
        let frame = session.frame()?;
        (frame.path.as_deref() == Some(path.as_path())).then_some(frame.line)
    }

    /// Sets a breakpoint on `line` of the file `pane` shows, or clears it.
    fn toggle_breakpoint(&mut self, pane: PaneId, line: usize) {
        let Some(file) = self
            .panes
            .pane(pane)
            .and_then(|pane| pane.active(self.scope()))
            .and_then(|item| self.file_in(item))
        else {
            return;
        };
        let (Some(scope), Some(path)) = (self.editor.scope_of(file), self.editor.path(file)) else {
            return;
        };
        self.debuggers.toggle(scope, &path, line);
    }

    /// Opens a breakpoint field prompt on the cursor's line.
    fn edit_cursor_breakpoint(&mut self, kind: Kind) {
        let Some(line) = self.with_buffer(|buffer| buffer.selection().head.line) else {
            return;
        };
        self.edit_breakpoint_at(self.panes.focus(), line, kind);
    }

    /// Opens a breakpoint field prompt for a pane's source line.
    pub(super) fn edit_breakpoint_at(&mut self, pane: PaneId, line: usize, kind: Kind) {
        let Some(file) = self
            .panes
            .pane(pane)
            .and_then(|pane| pane.active(self.scope()))
            .and_then(|item| self.file_in(item))
        else {
            return;
        };
        let (Some(scope), Some(path)) = (self.editor.scope_of(file), self.editor.path(file)) else {
            return;
        };
        let breakpoint = self.debuggers.breakpoint(scope, &path, line);
        let seeded = breakpoint
            .and_then(|breakpoint| match kind {
                Kind::BreakpointCondition => breakpoint.condition,
                Kind::BreakpointHits => breakpoint.hits,
                Kind::BreakpointLog => breakpoint.log,
                _ => None,
            })
            .unwrap_or_default();
        self.breakpoint_prompt = Some((scope, path, line));
        self.open_picker_with(kind, Vec::new(), seeded);
    }

    /// Saves one field of the breakpoint held by the prompt.
    pub(super) fn set_breakpoint_field(&mut self, kind: Kind, typed: String) {
        let Some((scope, path, line)) = self.breakpoint_prompt.take() else {
            return;
        };
        let mut breakpoint =
            self.debuggers
                .breakpoint(scope, &path, line)
                .unwrap_or(DapBreakpoint {
                    line,
                    ..DapBreakpoint::default()
                });
        let value = (!typed.is_empty()).then_some(typed);
        match kind {
            Kind::BreakpointCondition => breakpoint.condition = value,
            Kind::BreakpointHits => breakpoint.hits = value,
            Kind::BreakpointLog => breakpoint.log = value,
            _ => return,
        }
        self.debuggers.set_breakpoint(scope, &path, breakpoint);
    }

    /// Opens the watch prompt, seeded from a row or the selected text.
    pub(super) fn open_watch_prompt(&mut self, index: Option<usize>) {
        let Some(scope) = self.scope() else { return };
        let seeded = index
            .and_then(|index| self.debuggers.watches(scope).get(index).cloned())
            .or_else(|| self.with_buffer(pm_text::Buffer::selected_text))
            .unwrap_or_default();
        self.watch_prompt = Some((scope, index));
        self.open_picker_with(Kind::Watch, Vec::new(), seeded);
    }

    /// Adds or edits a watch expression in the worktree held by the prompt.
    pub(super) fn save_watch(&mut self, typed: String) {
        let Some((scope, index)) = self.watch_prompt.take() else {
            return;
        };
        if typed.trim().is_empty() {
            return;
        }
        let mut watches = self.debuggers.watches(scope).to_vec();
        if let Some(index) = index.and_then(|index| watches.get_mut(index)) {
            *index = typed;
        } else {
            watches.push(typed);
        }
        self.debuggers.set_watches(scope, watches);
    }

    /// Runs the paused program on, or starts debugging where nothing is.
    ///
    /// Starting asks what to debug unless there is only one thing it could
    /// be, which is what pressing one key to start is for.
    fn continue_or_start(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if let Some(session) = self.debuggers.live(scope) {
            if session.standing() == Standing::Stopped {
                session.resume();
            }
            return;
        }
        let rows = self.debug_rows();
        let only = match rows.as_slice() {
            [row] if row.enabled => Some(row.choice.clone()),
            _ => None,
        };
        match only {
            Some(Choice::Debug(scope, scenario)) => self.start_debugging(scope, *scenario),
            _ => self.open_picker_with(Kind::Debug, rows, String::new()),
        }
    }

    /// Debugs the worktree in front again as it was last debugged.
    fn restart_debugging(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        match self.debuggers.last(scope) {
            Some(scenario) => self.start_debugging(scope, scenario),
            None => self.open_picker(Kind::Debug),
        }
    }

    /// Carries `act` out on the program the worktree in front is debugging.
    fn with_session(&self, act: impl FnOnce(&pm_dap::Session)) {
        if let Some(session) = self.scope().and_then(|scope| self.debuggers.live(scope)) {
            act(session);
        }
    }

    /// Looks at the frame `frame` names, and brings up where it is.
    fn select_frame(&mut self, frame: i64) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(session) = self.debuggers.live(scope) else {
            return;
        };
        session.select_frame(frame);
        let found = session.frames().into_iter().find(|held| held.id == frame);
        if let Some(found) = found {
            self.bring_up(scope, &found);
        }
    }

    /// Opens the file `frame` is in at its line, in the pane in front.
    fn bring_up(&mut self, scope: Scope, frame: &pm_dap::Frame) {
        let Some(path) = frame.path.clone() else {
            return;
        };
        self.jump_to(&Place {
            scope,
            path,
            position: Position::new(frame.line, frame.column),
        });
    }

    /// Shows the debugger of `scope` in the bottom panel, the file staying
    /// where it was.
    fn show_debugger(&mut self, scope: Scope) {
        if self.scope() == Some(scope) {
            self.show_panel(PanelView::Debug);
        }
    }
}
