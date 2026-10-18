//! The terminal dock's state: the shells open in each session and what is done to them.

use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use pm_input::FocusId;
use pm_terminal::{Emulator, Link};

use crate::sample::Session;

use crate::state::{
    BOTTOM_PROBLEMS, BOTTOM_TERMINAL, MAXIMIZE_TERMINAL, NEW_TERMINAL, Outgoing, PROBLEM_ROW_BASE,
    TERMINAL, TERMINAL_CLOSE_BASE, TERMINAL_TAB_BASE, WorkspaceState,
};

/// The tabs of the dock, one of which shows at a time.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BottomTab {
    /// What is wrong with the files of the project.
    Problems,
    /// The program being debugged.
    Debug,
    /// The shells.
    #[default]
    Terminal,
}

/// How many columns and lines a shell has before the dock has been measured.
pub const FIRST_SIZE: (u16, u16) = (80, 24);

/// How many of the shell's lines one notch of the wheel scrolls.
pub const WHEEL_LINES: usize = 3;

/// An emulator that the state and what draws it share.
#[derive(Clone)]
pub struct Shared(Rc<RefCell<Emulator>>);

impl Shared {
    /// An emulator of `columns` by `rows`.
    fn new(columns: u16, rows: u16) -> Self {
        Self(Rc::new(RefCell::new(Emulator::new(
            columns as usize,
            rows as usize,
        ))))
    }

    /// Reads the emulator.
    pub fn read(&self) -> Ref<'_, Emulator> {
        self.0.borrow()
    }

    /// Changes the emulator.
    pub fn write(&self) -> RefMut<'_, Emulator> {
        self.0.borrow_mut()
    }
}

impl std::fmt::Debug for Shared {
    /// The emulator's size and version, which is all that tells two apart.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let held = self.read();
        write!(
            formatter,
            "Emulator({}x{} v{})",
            held.columns(),
            held.rows(),
            held.version()
        )
    }
}

impl PartialEq for Shared {
    /// Whether both are the same emulator.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// One shell of a session's dock.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalTab {
    /// What names it in this window, which the server's number for it is looked up by.
    pub id: u64,
    /// The number in its name, `Terminal 1` for one.
    pub label: usize,
    /// What it printed, as a window shows it.
    pub emulator: Shared,
    /// How many lines it is scrolled back from its end.
    pub back: usize,
    /// Whether the shell has ended.
    pub exited: bool,
    /// Text an input method is composing at the cursor, shown but not yet sent to the shell.
    pub composing: String,
    /// The link under the pointer while Ctrl is held, underlined as what a press opens.
    pub link: Option<Link>,
    /// What the task run in it is called, when it was started to run one.
    pub task: Option<String>,
}

impl TerminalTab {
    /// What the tab is called.
    pub fn name(&self) -> String {
        match &self.task {
            Some(task) => task.clone(),
            None => format!("Terminal {}", self.label),
        }
    }
}

impl WorkspaceState {
    /// Makes a tab for a new shell of the session at `session`, `columns` by `rows`, and makes it the one showing.
    pub(crate) fn add_terminal(&mut self, session: usize, columns: u16, rows: u16) -> Option<u64> {
        let id = self.next_terminal;
        let held = self.sessions.get_mut(session)?;
        let label = held
            .terminals
            .iter()
            .map(|tab| tab.label)
            .max()
            .unwrap_or(0)
            + 1;
        let emulator = Shared::new(columns, rows);
        emulator
            .write()
            .set_scrollback(self.window.terminal.scrollback);
        let held = self.sessions.get_mut(session)?;
        held.terminals.push(TerminalTab {
            id,
            label,
            emulator,
            back: 0,
            exited: false,
            composing: String::new(),
            link: None,
            task: None,
        });
        held.terminal_active = held.terminals.len() - 1;
        self.next_terminal += 1;
        Some(id)
    }

    /// Has every shell keep as many lines to scroll back to as the settings say.
    pub(crate) fn apply_scrollback(&mut self) {
        let lines = self.window.terminal.scrollback;
        for session in &self.sessions {
            for tab in &session.terminals {
                tab.emulator.write().set_scrollback(lines);
            }
        }
    }

    /// Starts a shell in the session the dock is about and shows the dock.
    fn open_terminal(&mut self) {
        self.bottom = BottomTab::Terminal;
        if let Some(session) = self.docked_position() {
            self.open_terminal_in(session, String::new());
        }
    }

    /// Starts a shell in `folder`, from the root of the folder of the session at `session`, and shows the dock.
    pub(crate) fn open_terminal_in(&mut self, session: usize, folder: String) {
        let (columns, rows) = self.terminal_size;
        if let Some(tab) = self.add_terminal(session, columns, rows) {
            self.terminal = true;
            self.outbox.push(Outgoing::OpenTerminal {
                session,
                folder,
                tab,
                columns,
                rows,
            });
        }
    }

    /// The position of the session the dock is about: the open one, or, on the page for starting work or a project's overview, the stand-in for the project's own folder.
    fn docked_position(&self) -> Option<usize> {
        self.review_index()
            .filter(|position| *position < self.sessions.len())
    }

    /// Whether the dock is about the session whose folder a program is debugged in, so that it has the debug tab.
    pub fn debug_docked(&self) -> bool {
        match (&self.debug.run, self.docked_position()) {
            (Some(run), Some(position)) => run.session == position,
            _ => false,
        }
    }

    /// The session the dock is about, if there is one.
    pub fn docked(&self) -> Option<&Session> {
        self.docked_position()
            .and_then(|position| self.sessions.get(position))
    }

    /// The tab of the dock showing: the one chosen, or the shells when the one chosen is the program being debugged and none is.
    pub fn bottom_tab(&self) -> BottomTab {
        match self.bottom {
            BottomTab::Debug if !self.debug_docked() => BottomTab::Terminal,
            tab => tab,
        }
    }

    /// Shows or hides the dock, starting a first shell when it is shown for a session with none and the shells are what it shows.
    fn toggle_terminal(&mut self) {
        if self.terminal {
            self.terminal = false;
            return;
        }
        let empty = self
            .open()
            .is_none_or(|session| session.terminals.is_empty());
        if empty && self.bottom_tab() == BottomTab::Terminal {
            self.open_terminal();
        } else {
            self.terminal = true;
        }
    }

    /// Shows the dock with the shells in it, starting one when the session the dock is about has none.
    fn show_terminal_tab(&mut self) {
        self.bottom = BottomTab::Terminal;
        let empty = self
            .docked()
            .is_none_or(|session| session.terminals.is_empty());
        if empty {
            self.open_terminal();
        } else {
            self.terminal = true;
        }
    }

    /// Shows the dock with the list of problems in it, asking the server what is wrong.
    pub fn show_problems(&mut self) {
        self.bottom = BottomTab::Problems;
        self.terminal = true;
        self.problems_scroll = 0.0;
        self.ask_problems();
    }

    /// Asks the server what is wrong with the files of the project of the session the dock is about, unless it was asked and has not answered.
    pub(crate) fn ask_problems(&mut self) {
        if self.problems_asked {
            return;
        }
        if let Some(session) = self.docked_position() {
            self.problems_asked = true;
            self.outbox.push(Outgoing::Problems { session });
        }
    }

    /// The state after the server said what is wrong with the files of the project of the session at `session`.
    pub fn with_problems(mut self, session: usize, problems: Vec<pm_protocol::Problem>) -> Self {
        self.problems_asked = false;
        if self.docked_position() == Some(session) {
            self.problems = problems;
        }
        self
    }

    /// Scrolls the list of problems by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_problems(mut self, delta: f32, room: f32) -> Self {
        self.problems_scroll = (self.problems_scroll + delta).clamp(0.0, room);
        self
    }

    /// Goes to the place of the problem the reader pressed in the list, when they pressed one.
    pub(crate) fn problem_revealed(mut self) -> Self {
        let Some(index) = self.problem_picked.take() else {
            return self;
        };
        let (Some(session), Some(problem)) =
            (self.docked_position(), self.problems.get(index).cloned())
        else {
            return self;
        };
        self.go_to_location(
            session,
            pm_protocol::Location {
                path: problem.path,
                span: problem.diagnostic.span,
                inside: true,
                ..pm_protocol::Location::default()
            },
        )
    }

    /// Ends the shell at `index` of the session the dock is about, hiding the dock when it was the last.
    pub(crate) fn close_terminal_tab(&mut self, index: usize) {
        let Some(position) = self.docked_position() else {
            return;
        };
        let session = &mut self.sessions[position];
        if index >= session.terminals.len() {
            return;
        }
        let tab = session.terminals.remove(index);
        session.terminal_active = session
            .terminal_active
            .min(session.terminals.len().saturating_sub(1));
        if session.terminals.is_empty() && self.bottom_tab() == BottomTab::Terminal {
            self.terminal = false;
            self.terminal_maximized = false;
        }
        self.outbox.push(Outgoing::CloseTerminal { tab: tab.id });
    }

    /// Carries out a press on one of the dock's controls, and says whether `id` was one.
    pub(crate) fn press_terminal(&mut self, id: FocusId) -> bool {
        match id {
            TERMINAL => self.toggle_terminal(),
            NEW_TERMINAL => self.open_terminal(),
            BOTTOM_TERMINAL => self.show_terminal_tab(),
            BOTTOM_PROBLEMS => self.show_problems(),
            MAXIMIZE_TERMINAL => self.terminal_maximized = !self.terminal_maximized,
            FocusId(number) => {
                let count = self.docked().map_or(0, |session| session.terminals.len());
                let offset = |base: u64| {
                    number
                        .checked_sub(base)
                        .map(|offset| offset as usize)
                        .filter(|offset| *offset < count)
                };
                if let Some(index) = offset(TERMINAL_TAB_BASE) {
                    self.bottom = BottomTab::Terminal;
                    if let Some(position) = self.docked_position() {
                        self.sessions[position].terminal_active = index;
                    }
                } else if let Some(index) = offset(TERMINAL_CLOSE_BASE) {
                    self.close_terminal_tab(index);
                } else if let Some(index) = number
                    .checked_sub(PROBLEM_ROW_BASE)
                    .map(|offset| offset as usize)
                    .filter(|offset| *offset < self.problems.len())
                {
                    self.problem_picked = Some(index);
                } else {
                    return false;
                }
            }
        }
        true
    }

    /// The shell showing for the session the dock is about.
    pub fn active_terminal(&self) -> Option<&TerminalTab> {
        let session = self.docked()?;
        session.terminals.get(session.terminal_active)
    }

    /// The shell showing for the session the dock is about, to change.
    pub(crate) fn active_terminal_mut(&mut self) -> Option<&mut TerminalTab> {
        let position = self.docked_position()?;
        let session = &mut self.sessions[position];
        session.terminals.get_mut(session.terminal_active)
    }

    /// Types `data` into the shell showing, which scrolls it back to its end.
    pub fn type_into_terminal(&mut self, data: Vec<u8>) {
        if let Some(tab) = self.active_terminal_mut() {
            tab.back = 0;
            tab.emulator.write().clear_selection();
            let id = tab.id;
            self.outbox.push(Outgoing::TerminalInput { tab: id, data });
        }
    }

    /// Shows `text` an input method is composing at the cursor of the shell showing, or stops showing it when empty.
    pub fn compose_in_terminal(&mut self, text: &str) {
        if let Some(tab) = self.active_terminal_mut() {
            tab.composing = text.chars().filter(|c| !c.is_control()).collect();
        }
    }

    /// Scrolls the shell showing `lines` lines back up, or down when negative.
    pub fn scroll_terminal(&mut self, lines: isize) {
        if let Some(tab) = self.active_terminal_mut() {
            let history = tab.emulator.read().history();
            tab.back = tab.back.saturating_add_signed(lines).min(history);
        }
    }

    /// Tells the shell showing, and the server, that its window is `columns` by `rows` now, when it was not that already.
    pub fn fit_terminal(&mut self, columns: u16, rows: u16) {
        self.terminal_size = (columns, rows);
        let Some(tab) = self.active_terminal_mut() else {
            return;
        };
        let (held_columns, held_rows) = {
            let held = tab.emulator.read();
            (held.columns(), held.rows())
        };
        if (held_columns, held_rows) == (columns as usize, rows as usize) {
            return;
        }
        tab.emulator.write().resize(columns as usize, rows as usize);
        tab.back = tab.back.min(tab.emulator.read().history());
        let id = tab.id;
        self.outbox.push(Outgoing::ResizeTerminal {
            tab: id,
            columns,
            rows,
        });
    }

    /// Reads what the shell `tab` printed, and answers what the program in it asked.
    pub fn terminal_output(&mut self, tab: u64, data: &[u8]) {
        let Some(found) = self
            .sessions
            .iter_mut()
            .flat_map(|session| session.terminals.iter_mut())
            .find(|held| held.id == tab)
        else {
            return;
        };
        let replies = {
            let mut emulator = found.emulator.write();
            emulator.feed(data);
            emulator.take_replies()
        };
        if found.back > 0 {
            found.back = found.back.min(found.emulator.read().history());
        }
        if !replies.is_empty() {
            self.outbox
                .push(Outgoing::TerminalInput { tab, data: replies });
        }
    }

    /// Takes the shell `tab` out of its session because it ended, hiding the dock when it was the last of the session the dock is about.
    pub fn terminal_exited(&mut self, tab: u64) {
        let open = self.docked_position();
        for (position, session) in self.sessions.iter_mut().enumerate() {
            let Some(index) = session.terminals.iter().position(|held| held.id == tab) else {
                continue;
            };
            session.terminals.remove(index);
            session.terminal_active = session
                .terminal_active
                .min(session.terminals.len().saturating_sub(1));
            if session.terminals.is_empty()
                && open == Some(position)
                && self.bottom_tab() == BottomTab::Terminal
            {
                self.terminal = false;
                self.terminal_maximized = false;
            }
            return;
        }
    }

    /// Adds a tab for a shell the server already runs for the session at `session`, and returns the name it is known by here.
    pub fn adopt_terminal(&mut self, session: usize, columns: u16, rows: u16) -> Option<u64> {
        let showing = self.terminal;
        let active = self.sessions.get(session)?.terminal_active;
        let had = !self.sessions.get(session)?.terminals.is_empty();
        let id = self.add_terminal(session, columns, rows)?;
        if had {
            self.sessions[session].terminal_active = active;
        }
        self.terminal = showing;
        Some(id)
    }

    /// The web address the cell at `at` of the window of the shell showing is part of, as a line of the window and a column.
    fn terminal_link_at(&self, at: (usize, usize)) -> Option<Link> {
        let tab = self.active_terminal()?;
        let emulator = tab.emulator.read();
        let line = emulator.first_line(tab.back) + at.0;
        emulator
            .link_at((line, at.1), pm_markdown::bare_address)
            .filter(|link| pm_acp::is_web_address(&link.target))
    }

    /// Underlines the web address at `at` of the window of the shell showing, or none when `at` is nothing or holds none.
    pub fn reach_terminal_link(&mut self, at: Option<(usize, usize)>) {
        let link = at.and_then(|at| self.terminal_link_at(at));
        if let Some(tab) = self.active_terminal_mut() {
            tab.link = link;
        }
    }

    /// Whether a web address of the shell showing is underlined as what a press opens.
    pub fn reaching_terminal_link(&self) -> bool {
        self.active_terminal().is_some_and(|tab| tab.link.is_some())
    }

    /// Opens in the browser the web address at `at` of the window of the shell showing, and says whether there was one.
    pub fn open_terminal_link(&mut self, at: (usize, usize)) -> bool {
        let Some(link) = self.terminal_link_at(at) else {
            return false;
        };
        self.outbox.push(Outgoing::Open {
            address: link.target,
        });
        true
    }

    /// The marked text of the shell showing.
    pub fn terminal_selection(&self) -> Option<String> {
        self.active_terminal()?.emulator.read().selected_text()
    }

    /// Marks text in the shell showing, from `anchor` to `head`, each as a line of the window and a column.
    pub fn select_in_terminal(&mut self, anchor: (usize, usize), head: (usize, usize)) {
        if let Some(tab) = self.active_terminal() {
            let first = {
                let emulator = tab.emulator.read();
                emulator.first_line(tab.back)
            };
            tab.emulator
                .write()
                .select((first + anchor.0, anchor.1), (first + head.0, head.1));
        }
    }

    /// Unmarks the text of the shell showing.
    pub fn clear_terminal_selection(&mut self) {
        if let Some(tab) = self.active_terminal() {
            tab.emulator.write().clear_selection();
        }
    }

    /// What the shell showing asks of the keys: whether the arrows send the application form, and whether a paste is wrapped in markers; nothing while no shell shows.
    pub fn terminal_modes(&self) -> Option<(bool, bool)> {
        if !self.terminal {
            return None;
        }
        let emulator = self.active_terminal()?.emulator.read();
        Some((emulator.application_cursor(), emulator.bracketed_paste()))
    }

    /// The position of the session on the machine `host` whose folder holds `directory`, the innermost when several do.
    pub fn session_in(&self, host: &Option<String>, directory: &str) -> Option<usize> {
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| !session.home)
            .filter_map(|(position, session)| {
                let project = self.projects.get(session.project)?;
                if project.host() != *host {
                    return None;
                }
                let root = if session.directory.is_empty() {
                    project.directory().to_string_lossy().into_owned()
                } else {
                    session.directory.clone()
                };
                within(directory, &root).then_some((position, root.len()))
            })
            .max_by_key(|(_, length)| *length)
            .map(|(position, _)| position)
    }

    /// Starts a shell in the session the dock is about when it is on the machine `host` and the dock shows none, as when the shells it showed ended with an earlier server.
    pub fn revive_terminal(&mut self, host: &Option<String>) {
        let Some(session) = self.docked() else {
            return;
        };
        if self.debug_docked() {
            return;
        }
        let on_host = self
            .projects
            .get(session.project)
            .is_some_and(|project| project.host() == *host);
        if self.terminal && on_host && session.terminals.is_empty() {
            self.open_terminal();
        }
    }

    /// Whether the dock shows a shell, and what its window holds: columns, lines and how far it is scrolled back.
    pub fn terminal_window(&self) -> Option<(usize, usize, usize)> {
        if self.bottom_tab() != BottomTab::Terminal {
            return None;
        }
        let tab = self.active_terminal()?;
        let emulator = tab.emulator.read();
        Some((emulator.columns(), emulator.rows(), tab.back))
    }

    /// Whether pressing `id` shows a shell that should then take the keyboard.
    pub fn takes_keyboard(id: FocusId) -> bool {
        id == TERMINAL
            || id == NEW_TERMINAL
            || id == BOTTOM_TERMINAL
            || (TERMINAL_TAB_BASE..TERMINAL_CLOSE_BASE).contains(&id.0)
    }
}

/// Whether `directory` is `root` or a folder inside it.
fn within(directory: &str, root: &str) -> bool {
    let root = root.trim_end_matches(['/', '\\']);
    directory
        .strip_prefix(root)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '\\']))
}
