//! Tasks as the workspace holds them: what the project can be built, tested and run with, the last one run, the toolchains installed on its machine, and what the reader does with them.

use pm_input::FocusId;
use pm_protocol::{Task, ToolchainInfo};
use pm_text::Field;

use crate::listing::{Items, Listing};
use crate::quick_choices::Choosing;
use crate::state::{Outgoing, WorkspaceState};

/// The first of the marks in the gutter that run a place of a file; each file shown has [`RUN_LINES`] after its own base.
pub const RUN_BASE: u64 = 6_000_000_000_000;

/// The first of the marks of the links at the ends of lines that run a place with one task of its language; each follows the base by its place in the list of links of the file shown.
pub const RUN_LENS_BASE: u64 = 8_700_000;

/// One past the last mark of those links.
pub const RUN_LENS_END: u64 = 8_800_000;

/// The mark of the link at `index` of the links of the file shown.
pub fn run_lens_mark(index: usize) -> FocusId {
    FocusId(RUN_LENS_BASE + index as u64)
}

/// The place in the list of links of the file shown that the mark `id` is.
pub fn run_lens_of(id: FocusId) -> Option<usize> {
    (RUN_LENS_BASE..RUN_LENS_END)
        .contains(&id.0)
        .then(|| (id.0 - RUN_LENS_BASE) as usize)
}

/// How many lines one file has room for among the run marks of the gutter.
pub const RUN_LINES: u64 = 1_000_000;

/// How many files have room among the run marks of the gutter.
const RUN_FILES: u64 = 1000;

/// The mark in the gutter of the line `line` of the open file at `file` that runs the place that starts there.
pub fn run_mark(file: usize, line: usize) -> FocusId {
    FocusId(RUN_BASE + file as u64 * RUN_LINES + line as u64)
}

/// The open file and the line the gutter run mark `id` is on.
pub fn run_of(id: FocusId) -> Option<(usize, usize)> {
    let offset =
        id.0.checked_sub(RUN_BASE)
            .filter(|offset| *offset < RUN_FILES * RUN_LINES)?;
    Some(((offset / RUN_LINES) as usize, (offset % RUN_LINES) as usize))
}

/// What was last run, which running it again does as it was done.
#[derive(Clone, Debug, PartialEq)]
pub enum Rerun {
    /// A task of the project.
    Task(Task),
    /// A place of a file that a language marks as runnable.
    Runnable {
        /// The file, from the root of the session's folder.
        path: String,
        /// What its language tagged it with.
        tags: Vec<String>,
        /// What each of its captures matched, by the capture's name.
        variables: std::collections::BTreeMap<String, String>,
        /// Its line and its column, from zero.
        position: (usize, usize),
        /// The name of the task tied to it that is run in place of the first, when one was chosen.
        task: Option<String>,
    },
}

impl Rerun {
    /// What the tab that runs it is titled: the first thing its place captured, or its first tag.
    fn title(&self) -> String {
        match self {
            Self::Task(task) => task.name.clone(),
            Self::Runnable {
                tags, variables, ..
            } => variables
                .values()
                .next()
                .or(tags.first())
                .cloned()
                .unwrap_or_default(),
        }
    }
}

/// What the workspace holds of tasks and toolchains.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskState {
    /// What the folder of the session being worked in can be built, tested and run with, as the server last listed it.
    pub tasks: Vec<Task>,
    /// Why the project's own task file could not be read, when it could not.
    pub problem: Option<String>,
    /// The toolchains installed, as the server last listed them.
    pub toolchains: Vec<ToolchainInfo>,
    /// What was run last.
    pub last: Option<Rerun>,
    /// The place of a file whose run button was pressed, while the server says which tasks can run it and the reader picks one.
    pub waiting: Option<Rerun>,
    /// The tasks that can run that place, which the reader picks from when there is more than one.
    pub options: Vec<pm_protocol::RunnableTask>,
}

/// The language `family` as a reader knows it: what its manifest calls it, or the family as it is written.
fn language_name(family: &str, named: &str) -> String {
    if named.is_empty() { family } else { named }.to_owned()
}

impl WorkspaceState {
    /// Asks the server what the folder of the session being worked in can be built, tested and run with.
    pub(crate) fn tasks_asked(&mut self) {
        if let Some(session) = self.review_index() {
            self.outbox.push(Outgoing::Tasks { session });
        }
    }

    /// The state after the server listed the tasks of the folder of the session at `session`, and why the project's own file could not be read when it could not.
    pub fn with_tasks(mut self, session: usize, tasks: Vec<Task>, problem: Option<String>) -> Self {
        if self.review_index() == Some(session) {
            self.tasks.tasks = tasks;
            self.tasks.problem = problem;
            self.commands_renewed();
            self.choices_renewed(Choosing::Task);
            if let Some(listing) = self.listing.as_mut()
                && listing.session == session
            {
                listing.answered = true;
                if matches!(
                    listing.items,
                    Items::Choices {
                        choosing: Choosing::Task,
                        ..
                    }
                ) {
                    listing.problem = self.tasks.problem.clone();
                }
            }
        }
        self
    }

    /// Runs `how` in a new shell of the dock, in the folder of the session being worked in, and shows the dock.
    fn run_in_terminal(&mut self, how: Rerun) {
        let Some(session) = self.review_index() else {
            return;
        };
        let (columns, rows) = self.terminal_size;
        let Some(tab) = self.add_terminal(session, columns, rows) else {
            return;
        };
        let title = how.title();
        if let Some(held) = self
            .sessions
            .get_mut(session)
            .and_then(|held| held.terminals.iter_mut().find(|held| held.id == tab))
        {
            held.task = Some(title);
        }
        self.bottom = crate::dock::BottomTab::Terminal;
        self.terminal = true;
        self.outbox.push(match how.clone() {
            Rerun::Task(task) => Outgoing::RunTask {
                session,
                task,
                tab,
                columns,
                rows,
            },
            Rerun::Runnable {
                path,
                tags,
                variables,
                position,
                task,
            } => Outgoing::RunRunnable {
                session,
                path,
                tags,
                variables,
                position,
                task,
                tab,
                columns,
                rows,
            },
        });
        self.tasks.last = Some(how);
    }

    /// Runs `task` in the dock.
    pub(crate) fn task_run(&mut self, task: Task) {
        self.run_in_terminal(Rerun::Task(task));
    }

    /// Runs the task at `index` of those listed in the dock.
    pub(crate) fn task_taken(&mut self, index: usize) {
        if let Some(task) = self.tasks.tasks.get(index).cloned() {
            self.task_run(task);
        }
    }

    /// Runs again what was run last, when something was.
    pub fn task_rerun(mut self) -> Self {
        if let Some(last) = self.tasks.last.clone() {
            self.run_in_terminal(last);
        }
        self
    }

    /// Runs the place that starts on `line` of the open file at `file`, when the language marks one there.
    pub(crate) fn runnable_run(&mut self, file: usize, line: usize) {
        let Some(viewer) = self.git.viewers.get(file).filter(|viewer| !viewer.head) else {
            return;
        };
        let path = viewer.path.clone();
        let Some(place) = self.shown_runnable(file, line) else {
            return;
        };
        let Some(session) = self.review_index() else {
            return;
        };
        self.outbox.push(Outgoing::RunnableTasks {
            session,
            path: path.clone(),
            tags: place.tags.clone(),
            variables: place.variables.clone(),
            position: (line, 0),
        });
        self.tasks.waiting = Some(Rerun::Runnable {
            path,
            tags: place.tags,
            variables: place.variables,
            position: (line, 0),
            task: None,
        });
    }

    /// Runs the place of the link at `index` of those the file shown has at the ends of its lines, with the task the link names.
    pub(crate) fn run_lens_pressed(&mut self, index: usize) {
        let Some(path) = self.shown_viewer().map(|viewer| viewer.path.clone()) else {
            return;
        };
        let Some(item) = self
            .shown_document()
            .and_then(|document| document.run_lens_items().into_iter().nth(index))
        else {
            return;
        };
        self.run_in_terminal(Rerun::Runnable {
            path,
            tags: item.place.tags,
            variables: item.place.variables,
            position: (item.line, 0),
            task: Some(item.label),
        });
    }

    /// The state after the server listed the tasks that can run the place whose button was pressed: the first runs at once when it is the only one, and otherwise the reader is asked which to run.
    pub fn with_runnable_tasks(
        mut self,
        session: usize,
        tasks: Vec<pm_protocol::RunnableTask>,
    ) -> Self {
        if self.review_index() != Some(session) || self.tasks.waiting.is_none() {
            return self;
        }
        if tasks.len() <= 1 {
            if let Some(place) = self.tasks.waiting.take() {
                self.run_in_terminal(place);
            }
            return self;
        }
        self.tasks.options = tasks;
        self.choices_requested(crate::quick_choices::Choosing::RunnableTask, (340.0, 0.0))
    }

    /// Runs the place whose button was pressed with the task at `index` of those the reader was offered.
    pub(crate) fn runnable_taken(&mut self, index: usize) {
        let Some(Rerun::Runnable {
            path,
            tags,
            variables,
            position,
            ..
        }) = self.tasks.waiting.take()
        else {
            return;
        };
        let Some(chosen) = self.tasks.options.get(index) else {
            return;
        };
        self.run_in_terminal(Rerun::Runnable {
            path,
            tags,
            variables,
            position,
            task: Some(chosen.name.clone()),
        });
    }

    /// The runnable place that starts on `line` of the open file at `file`.
    fn shown_runnable(&self, file: usize, line: usize) -> Option<pm_treesitter::Runnable> {
        let document = match &self.git.viewers.get(file)?.contents {
            crate::open_files::Contents::Text(document) => document,
            _ => return None,
        };
        document
            .runnables()
            .iter()
            .find(|(held, place)| *held == line && !document.is_lens_place(place))
            .map(|(_, place)| place.clone())
    }

    /// The lines of the open file at `file` where a place that can be run starts.
    pub fn run_lines_of(&self, file: usize) -> Vec<usize> {
        let Some(viewer) = self.git.viewers.get(file).filter(|viewer| !viewer.head) else {
            return Vec::new();
        };
        match &viewer.contents {
            crate::open_files::Contents::Text(document) => {
                let mut lines: Vec<usize> = document
                    .runnables()
                    .iter()
                    .filter(|(_, place)| !document.is_lens_place(place))
                    .map(|(line, _)| *line)
                    .collect();
                lines.dedup();
                lines
            }
            _ => Vec::new(),
        }
    }

    /// Lists the tasks of the folder of the session being worked in at `at`, to run one.
    pub fn tasks_requested(mut self, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        self.tasks_asked();
        let answered = !self.tasks.tasks.is_empty();
        let mut opened = self.choices_requested(Choosing::Task, at);
        if let Some(listing) = opened.listing.as_mut() {
            listing.answered = answered;
            listing.session = session;
        }
        opened
    }

    /// Lists the toolchains installed on the machine of the session being worked in at `at`, to choose the one a language is run with.
    pub fn toolchains_requested(mut self, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        self.outbox.push(Outgoing::Toolchains { session });
        self.listing = Some(Listing {
            session,
            items: Items::Choices {
                choosing: Choosing::Toolchain,
                choices: Vec::new(),
                filter: Field::new(""),
                matched: Vec::new(),
            },
            picked: 0,
            answered: false,
            problem: None,
            at,
            quick: true,
        });
        self
    }

    /// The state after the server listed the toolchains of the machine of the session at `session`.
    pub fn with_toolchains(mut self, session: usize, toolchains: Vec<ToolchainInfo>) -> Self {
        self.tasks.toolchains = toolchains;
        self.choices_renewed(Choosing::Toolchain);
        if let Some(listing) = self.listing.as_mut()
            && listing.session == session
        {
            listing.answered = true;
        }
        self
    }

    /// Has the project of the session being worked in run `language` with the toolchain `id`, or with the machine's own path when there is none.
    pub(crate) fn toolchain_taken(&mut self, language: String, id: Option<String>) {
        let Some(session) = self.review_index() else {
            return;
        };
        for held in &mut self.tasks.toolchains {
            if held.language == language {
                held.selected = held.id == id.as_deref().unwrap_or("system");
            }
        }
        self.outbox.push(Outgoing::SelectToolchain {
            session,
            language,
            toolchain: id,
        });
    }

    /// The name `language`, by family, is shown under in the list of toolchains.
    pub(crate) fn toolchain_language(language: &str, named: &str) -> String {
        language_name(language, named)
    }
}
