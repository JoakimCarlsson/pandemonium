//! The breakpoints the window keeps, and the programs it is debugging.
//!
//! [`Debuggers`] is the one seam a debugging session is started and ended
//! through, and it is keyed by worktree the way the shells are: a program is
//! debugged in the worktree it was built from, and the breakpoints it stops
//! on are that worktree's. One worktree debugs one program at a time; a
//! session cut beside a checkout debugs its own, with its own breakpoints.
//!
//! [`Debugger`] is one of them — the session, the box expressions are typed
//! into, and which of its variables the reader has opened.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pm_core::Scope;
use pm_dap::{Breakpoint, Event, Notify, Scenario, Session, Standing};
use pm_gfx::Point;
use pm_ui::{Bounds, Scrolled};

use crate::input::Input;

/// One program being debugged, and what the reader has opened of it.
pub struct Debugger {
    /// The session itself, as the protocol carries it.
    session: Session,
    /// The box expressions are typed into.
    console: Input,
    /// The variables and scopes the reader has opened, by what their
    /// members are asked for by.
    opened: BTreeSet<i64>,
    /// The scopes the reader has closed, by name, which stay closed from one
    /// pause to the next.
    closed: BTreeSet<String>,
    /// How far the call stack is scrolled.
    stack_scroll: Scrolled,
    /// How far the variables are scrolled.
    variables_scroll: Scrolled,
    /// Where the call stack came out in the last frame.
    stack_area: Bounds,
    /// Where the variables came out in the last frame.
    variables_area: Bounds,
    /// Where the console's lines came out in the last frame.
    console_area: Bounds,
    /// How many lines back from its end the console is scrolled.
    console_back: usize,
}

impl Debugger {
    /// The session itself.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The box expressions are typed into.
    pub fn console(&self) -> &Input {
        &self.console
    }

    /// That box, to write in.
    pub fn console_mut(&mut self) -> &mut Input {
        &mut self.console
    }

    /// Whether the variable or scope behind `reference` is open.
    pub fn is_open(&self, reference: i64) -> bool {
        self.opened.contains(&reference)
    }

    /// Whether the scope called `name` is open, which it is unless the
    /// reader closed it; registers start closed, being many and seldom read.
    pub fn is_scope_open(&self, name: &str) -> bool {
        !self.closed.contains(name)
    }

    /// How far the call stack is scrolled.
    pub fn stack_scroll(&self) -> Scrolled {
        self.stack_scroll.clone()
    }

    /// How far the variables are scrolled.
    pub fn variables_scroll(&self) -> Scrolled {
        self.variables_scroll.clone()
    }

    /// Where the call stack came out in the last frame.
    pub fn stack_area(&self) -> Bounds {
        self.stack_area.clone()
    }

    /// Where the variables came out in the last frame.
    pub fn variables_area(&self) -> Bounds {
        self.variables_area.clone()
    }

    /// Where the console's lines came out in the last frame.
    pub fn console_area(&self) -> Bounds {
        self.console_area.clone()
    }

    /// How many lines back from its end the console is scrolled.
    pub fn console_back(&self) -> usize {
        self.console_back
    }

    /// Scrolls whichever of the pane's lists is under `pointer` by `delta`
    /// logical pixels, `line` of them to a line of the console, answering
    /// whether one was.
    ///
    /// The console counts back from its end, so it follows what the program
    /// writes until the reader scrolls up, and follows it again once they
    /// have scrolled back down.
    pub fn scroll(&mut self, pointer: Point, delta: f32, line: f32) -> bool {
        for (area, scroll) in [
            (&self.stack_area, &self.stack_scroll),
            (&self.variables_area, &self.variables_scroll),
        ] {
            if area.get().contains(pointer) {
                let mut moved = scroll.get();
                moved.by(delta);
                scroll.set(moved);
                return true;
            }
        }
        if !self.console_area.get().contains(pointer) {
            return false;
        }
        let lines = (delta / line.max(1.0)).round() as isize;
        let most = self.session.line_count().saturating_sub(1);
        self.console_back = self.console_back.saturating_add_signed(lines).min(most);
        true
    }

    /// Opens the variable behind `reference`, or closes it.
    pub fn toggle(&mut self, reference: i64) {
        if !self.opened.remove(&reference) {
            self.opened.insert(reference);
            self.session.expand(reference);
        }
    }

    /// Opens the scope called `name`, or closes it.
    pub fn toggle_scope(&mut self, name: &str) {
        if !self.closed.remove(name) {
            self.closed.insert(name.to_owned());
        }
    }

    /// Asks what the console holds, and empties it.
    pub fn evaluate(&mut self) {
        let expression = self.console.value();
        let expression = expression.trim();
        if expression.is_empty() {
            return;
        }
        self.session.evaluate(expression);
        self.console.clear();
    }

    /// Forgets what was opened at the last pause: a variable is named by a
    /// number that lasts only as long as the pause it was read in.
    fn paused(&mut self) {
        self.opened.clear();
    }
}

/// Every breakpoint the window keeps, and every program it is debugging.
#[derive(Default)]
pub struct Debuggers {
    /// The lines each file of each worktree stops on, counted from zero.
    breakpoints: BTreeMap<Scope, BTreeMap<PathBuf, BTreeMap<usize, Breakpoint>>>,
    /// Watch expressions belonging to each worktree.
    watches: BTreeMap<Scope, Vec<String>>,
    /// The program each worktree is debugging.
    running: BTreeMap<Scope, Debugger>,
    /// What each worktree was last debugged as, to debug it as again.
    last: BTreeMap<Scope, Scenario>,
    /// How a session wakes the window.
    notify: Option<Notify>,
}

impl Debuggers {
    /// Wakes the window through `notify` when a session says something.
    pub fn set_notify(&mut self, notify: Notify) {
        self.notify = Some(notify);
    }

    /// The full breakpoints of a file in one worktree.
    pub fn breakpoints(&self, scope: Scope, path: &Path) -> Vec<Breakpoint> {
        self.breakpoints
            .get(&scope)
            .and_then(|files| files.get(path))
            .map(|lines| lines.values().cloned().collect())
            .unwrap_or_default()
    }

    /// A breakpoint on one line, if present.
    pub fn breakpoint(&self, scope: Scope, path: &Path, line: usize) -> Option<Breakpoint> {
        self.breakpoints.get(&scope)?.get(path)?.get(&line).cloned()
    }

    /// Adds or edits a breakpoint, immediately telling a running adapter.
    pub fn set_breakpoint(&mut self, scope: Scope, path: &Path, breakpoint: Breakpoint) {
        self.breakpoints
            .entry(scope)
            .or_default()
            .entry(path.to_path_buf())
            .or_default()
            .insert(breakpoint.line, breakpoint);
        self.tell(scope, path);
    }

    /// The watches of one worktree.
    pub fn watches(&self, scope: Scope) -> &[String] {
        self.watches
            .get(&scope)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Replaces the watches of one worktree and updates its running session.
    pub fn set_watches(&mut self, scope: Scope, watches: Vec<String>) {
        self.watches.insert(scope, watches.clone());
        if let Some(debugger) = self.running.get(&scope) {
            debugger.session.set_watches(watches);
        }
    }

    /// Sets a breakpoint on `line` of `path` in `scope`, or clears the one
    /// there, telling the program being debugged there at once.
    pub fn toggle(&mut self, scope: Scope, path: &Path, line: usize) {
        let files = self.breakpoints.entry(scope).or_default();
        let lines = files.entry(path.to_path_buf()).or_default();
        if lines.remove(&line).is_none() {
            lines.insert(
                line,
                Breakpoint {
                    line,
                    ..Breakpoint::default()
                },
            );
        }
        if lines.is_empty() {
            files.remove(path);
        }
        self.tell(scope, path);
    }

    /// Clears every breakpoint of `scope`.
    pub fn clear(&mut self, scope: Scope) {
        let cleared = self.breakpoints.remove(&scope).unwrap_or_default();
        for path in cleared.keys() {
            self.tell(scope, path);
        }
    }

    /// Starts debugging `scenario` in `scope`, whose worktree is `root`,
    /// ending what was being debugged there before.
    ///
    /// The answer is why it would not start, which is the reader's to see:
    /// an adapter that is not installed is something they can fix.
    pub fn start(&mut self, scope: Scope, root: &Path, scenario: Scenario) -> Result<(), String> {
        let notify = self
            .notify
            .clone()
            .ok_or_else(|| "the window is not ready".to_owned())?;
        self.running.remove(&scope);
        let breakpoints = self
            .breakpoints
            .get(&scope)
            .map(|files| {
                files
                    .iter()
                    .map(|(path, lines)| (path.clone(), lines.values().cloned().collect()))
                    .collect()
            })
            .unwrap_or_default();
        let session = Session::start(scenario.clone(), root, breakpoints, notify)
            .map_err(|error| error.to_string())?;
        session.set_watches(self.watches(scope).to_vec());
        self.last.insert(scope, scenario);
        self.running.insert(
            scope,
            Debugger {
                session,
                console: Input::one_line("Debug Console"),
                opened: BTreeSet::new(),
                closed: BTreeSet::from(["Registers".to_owned()]),
                stack_scroll: Scrolled::default(),
                variables_scroll: Scrolled::default(),
                stack_area: Bounds::default(),
                variables_area: Bounds::default(),
                console_area: Bounds::default(),
                console_back: 0,
            },
        );
        Ok(())
    }

    /// What `scope` was last debugged as.
    pub fn last(&self, scope: Scope) -> Option<Scenario> {
        self.last.get(&scope).cloned()
    }

    /// The program `scope` is debugging, running, paused or ended.
    pub fn get(&self, scope: Scope) -> Option<&Debugger> {
        self.running.get(&scope)
    }

    /// That program, to act on.
    pub fn get_mut(&mut self, scope: Scope) -> Option<&mut Debugger> {
        self.running.get_mut(&scope)
    }

    /// The program `scope` is debugging, while it has not ended.
    pub fn live(&self, scope: Scope) -> Option<&Session> {
        self.running
            .get(&scope)
            .map(Debugger::session)
            .filter(|session| session.standing() != Standing::Ended)
    }

    /// Ends what `scope` is debugging.
    pub fn stop(&mut self, scope: Scope) {
        if let Some(debugger) = self.running.get(&scope) {
            debugger.session.stop();
        }
    }

    /// Forgets every worktree `leaving` names, ending what each is
    /// debugging.
    pub fn forget(&mut self, leaving: impl Fn(Scope) -> bool) {
        self.running.retain(|scope, _| !leaving(*scope));
        self.breakpoints.retain(|scope, _| !leaving(*scope));
        self.watches.retain(|scope, _| !leaving(*scope));
        self.last.retain(|scope, _| !leaving(*scope));
    }

    /// Takes in what every session has said, answering whether anything
    /// has and what the window has to act on.
    pub fn pump(&mut self) -> (bool, Vec<(Scope, Event)>) {
        let mut changed = false;
        let mut events = Vec::new();
        for (scope, debugger) in &mut self.running {
            changed |= debugger.session.take_fresh();
            for event in debugger.session.take_events() {
                if matches!(event, Event::Paused(_)) {
                    debugger.paused();
                }
                events.push((*scope, event));
            }
        }
        (changed || !events.is_empty(), events)
    }

    /// Tells the program being debugged in `scope` what the breakpoints of
    /// `path` now are.
    fn tell(&self, scope: Scope, path: &Path) {
        if let Some(debugger) = self.running.get(&scope) {
            debugger
                .session
                .set_breakpoints(path, self.breakpoints(scope, path));
        }
    }
}
