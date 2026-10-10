//! Test commands routed through task execution, source navigation and debugging.

use crate::app::{App, places::Place};
use crate::message::Message;
use crate::panes::{Item, Tool};
use crate::tasks::{Outcome, Shown};
use crate::testing::{Command, Pending, TestRun, Worktree};
use pm_core::{
    Scope,
    testing::{Case, Coverage, Python, Selection, SourceRevision, Status},
};
use pm_text::Position;
use pm_ui::{Div, Theme};

impl App {
    /// Handles explorer commands against their originating project and worktree.
    pub(super) fn apply_testing(&mut self, message: Message) -> bool {
        let Message::Test(scope, command) = message else {
            return false;
        };
        if self.root_of(scope).is_none() {
            return true;
        }
        self.testing.worktrees.entry(scope).or_default();
        match command {
            Command::Refresh => {
                self.testing.worktrees.get_mut(&scope).unwrap().history = false;
                self.testing.worktrees.get_mut(&scope).unwrap().refresh = true;
                self.refresh_tests(scope);
            }
            Command::All | Command::Suite(_) | Command::Case(_) | Command::Cover => {
                self.run_tests(scope, command)
            }
            Command::Cancel => {
                if let Some(run) = self.testing.worktrees[&scope].active() {
                    self.tasks.stop(run);
                    self.hear_finished_tasks();
                }
            }
            Command::Source(index) => {
                let tree = self.testing.worktrees.get_mut(&scope).unwrap();
                let location = tree
                    .shown()
                    .get(index)
                    .and_then(|case| case.failure.clone().or_else(|| case.location.clone()));
                tree.selected_case = Some(index);
                if let Some(location) = location {
                    self.point_at(scope);
                    self.jump_to(&Place {
                        scope,
                        path: location.path,
                        position: Position::new(location.line, 0),
                    });
                }
            }
            Command::Debug(index) => self.debug_test(scope, index),
            Command::Import => {
                let root = self.root_of(scope).unwrap();
                self.import_test_coverage(scope, &root.join("coverage.lcov"));
            }
            Command::Clear => self.testing.worktrees.get_mut(&scope).unwrap().coverage = None,
            Command::CoverageSource(index) => {
                let file = self.testing.worktrees[&scope]
                    .coverage
                    .as_ref()
                    .and_then(|coverage| coverage.files.get(index));
                if let Some(file) = file {
                    let line = file
                        .lines
                        .iter()
                        .find(|(_, count)| **count == 0)
                        .map_or(0, |(line, _)| *line);
                    let place = Place {
                        scope,
                        path: file.path.clone(),
                        position: Position::new(line, 0),
                    };
                    self.point_at(scope);
                    self.jump_to(&place);
                }
            }
            Command::History(index) => {
                let tree = self.testing.worktrees.get_mut(&scope).unwrap();
                if index < tree.runs.len() {
                    tree.selected_run = Some(index);
                    tree.history = true;
                    tree.selected_case = None;
                }
            }
            Command::Output => {
                let run = self.testing.worktrees[&scope]
                    .selected_run
                    .and_then(|index| self.testing.worktrees[&scope].runs.get(index))
                    .map(|run| run.task);
                if let Some(run) = run {
                    self.point_at(scope);
                    self.tasks.show(&mut self.terminals, run);
                    self.show_panel(crate::panel::PanelView::Terminal);
                }
            }
        }
        true
    }

    /// Allocates journals under the user's cache, never in a temporary RAM filesystem.
    fn test_directory(&mut self) -> Result<std::path::PathBuf, String> {
        let cache = std::env::var_os("XDG_CACHE_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::home_dir().map(|home| home.join(".cache")))
            .ok_or("No user cache directory")?;
        let path = cache.join("pandemonium/tests").join(format!(
            "{}-{}-{}",
            std::process::id(),
            self.testing.next,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        self.testing.next += 1;
        Ok(path)
    }

    /// Builds a framework task plan and removes partial artifacts on planning errors.
    fn plan_tests(
        &mut self,
        root: &std::path::Path,
        selection: Option<Selection>,
        coverage: bool,
    ) -> Result<(std::path::PathBuf, pm_core::Task), String> {
        let adapter = Python::read(root)?;
        let directory = self.test_directory()?;
        match adapter.task(&directory, selection, coverage) {
            Ok(task) => Ok((directory, task)),
            Err(error) => {
                let _ = std::fs::remove_dir_all(directory);
                Err(error)
            }
        }
    }

    /// Starts a pending discovery through the shared worktree task seam.
    pub(super) fn refresh_tests(&mut self, scope: Scope) {
        let Some(tree) = self.testing.worktrees.get(&scope) else {
            return;
        };
        if !tree.refresh || tree.active().is_some() {
            return;
        }
        let Some(root) = self.root_of(scope) else {
            return;
        };
        let planned = self.plan_tests(&root, None, false);
        match planned {
            Ok((directory, task)) => {
                if let Some(run) = self.run_task(scope, &task, Shown::Quiet) {
                    let tree = self.testing.worktrees.get_mut(&scope).unwrap();
                    tree.discovery = Some(Pending {
                        task: run,
                        directory,
                        epoch: tree.epoch,
                    });
                    tree.refresh = false;
                    tree.error.clear();
                } else {
                    let _ = std::fs::remove_dir_all(directory);
                    self.testing.worktrees.get_mut(&scope).unwrap().refresh = false;
                }
            }
            Err(error) => {
                let tree = self.testing.worktrees.get_mut(&scope).unwrap();
                tree.error = error;
                tree.refresh = false;
            }
        }
    }

    /// Starts all, suite or case execution using normalized discovered identities.
    fn run_tests(&mut self, scope: Scope, command: Command) {
        let tree = &self.testing.worktrees[&scope];
        if tree.active().is_some() {
            return;
        }
        let selection = match command {
            Command::Suite(index) => tree
                .shown()
                .get(index)
                .map(|case| Selection::Suite(case.suite.clone())),
            Command::Case(index) => tree
                .shown()
                .get(index)
                .map(|case| Selection::Case(case.id.clone())),
            _ => Some(Selection::All),
        };
        let Some(selection) = selection else { return };
        let cases: Vec<Case> = tree
            .cases
            .iter()
            .filter(|case| match &selection {
                Selection::All => true,
                Selection::Suite(suite) => &case.suite == suite,
                Selection::Case(id) => &case.id == id,
            })
            .cloned()
            .map(|mut case| {
                case.status = Status::Queued;
                case.output.clear();
                case.failure = None;
                case.duration = 0.0;
                case
            })
            .collect();
        let coverage = command == Command::Cover;
        let Some(root) = self.root_of(scope) else {
            return;
        };
        let revision = SourceRevision::capture(&root);
        let planned = self.plan_tests(&root, Some(selection), coverage);
        match planned {
            Ok((directory, task)) => {
                if let Some(run) = self.run_task(scope, &task, Shown::Quiet) {
                    let tree = self.testing.worktrees.get_mut(&scope).unwrap();
                    tree.selected_run = Some(tree.runs.len());
                    tree.selected_case = None;
                    tree.history = false;
                    tree.error.clear();
                    for case in &cases {
                        if let Some(held) = tree.cases.iter_mut().find(|held| held.id == case.id) {
                            *held = case.clone();
                        }
                    }
                    tree.runs.push(TestRun {
                        task: run,
                        directory,
                        cases,
                        revision,
                        outcome: None,
                        coverage,
                        epoch: tree.epoch,
                        output: String::new(),
                    });
                } else {
                    let _ = std::fs::remove_dir_all(directory);
                }
            }
            Err(error) => self.testing.worktrees.get_mut(&scope).unwrap().error = error,
        }
    }

    /// Normalizes discovery and case journals while preserving the task runner's final outcome.
    pub(super) fn take_test_results(&mut self) {
        let scopes = self.testing.worktrees.keys().copied().collect::<Vec<_>>();
        for scope in scopes {
            let root = self.root_of(scope);
            let mut coverage = None;
            let tree = self.testing.worktrees.get_mut(&scope).unwrap();
            if let Some(pending) = &tree.discovery
                && let Some(outcome) = self.tasks.outcome(pending.task)
            {
                let pending = tree.discovery.take().unwrap();
                if outcome == Outcome::Succeeded {
                    match std::fs::read_to_string(pending.directory.join("discovery.json"))
                        .map_err(|error| error.to_string())
                        .and_then(|text| {
                            serde_json::from_str::<Vec<Case>>(&text)
                                .map_err(|error| error.to_string())
                        }) {
                        Ok(mut cases) if pending.epoch == tree.epoch => {
                            for case in &mut cases {
                                if let Some(old) = tree.cases.iter().find(|old| old.id == case.id) {
                                    case.status = old.status;
                                    case.duration = old.duration;
                                    case.output.clone_from(&old.output);
                                    case.failure.clone_from(&old.failure);
                                }
                            }
                            tree.cases = cases;
                        }
                        Ok(_) => tree.refresh = true,
                        Err(error) => tree.error = error,
                    }
                } else {
                    tree.error = format!(
                        "Discovery {outcome:?}\n{}",
                        self.tasks.tail(pending.task, 200)
                    );
                }
                let _ = std::fs::remove_dir_all(pending.directory);
            }
            for run in &mut tree.runs {
                if run.outcome.is_some() {
                    continue;
                }
                run.outcome = self.tasks.outcome(run.task);
                if run.outcome.is_some() {
                    run.output = self.tasks.tail(run.task, 500);
                }
                run.read();
                for case in &run.cases {
                    if let Some(held) = tree.cases.iter_mut().find(|held| held.id == case.id) {
                        *held = case.clone();
                    }
                }
                if run.outcome.is_some() && run.coverage {
                    let report = run.directory.join("coverage.lcov");
                    if report.is_file() {
                        coverage = Some((
                            report,
                            run.epoch,
                            root.as_ref().is_some_and(|root| run.revision.current(root)),
                        ));
                    } else {
                        tree.error = format!(
                            "Coverage was not produced: {:?}\n{}",
                            run.outcome, run.output
                        );
                    }
                }
            }
            if let Some((report, epoch, current)) = coverage {
                self.import_test_coverage(scope, &report);
                if let Some(tree) = self.testing.worktrees.get_mut(&scope)
                    && (tree.epoch != epoch || !current)
                    && let Some(coverage) = &mut tree.coverage
                {
                    coverage.invalidate();
                }
            }
        }
    }

    /// Imports a report explicitly attached to the worktree's current saved source revision.
    fn import_test_coverage(&mut self, scope: Scope, report: &std::path::Path) {
        let Some(root) = self.root_of(scope) else {
            return;
        };
        let result = Coverage::read(&root, report);
        let tree = self.testing.worktrees.get_mut(&scope).unwrap();
        match result {
            Ok(coverage) => {
                tree.coverage = Some(coverage);
                tree.error.clear();
            }
            Err(error) => tree.error = format!("Coverage import failed: {error}"),
        }
    }

    /// Debugs one discovered unittest identity with the existing breakpoint and DAP seam.
    fn debug_test(&mut self, scope: Scope, index: usize) {
        let Some(case) = self.testing.worktrees[&scope].shown().get(index) else {
            return;
        };
        if case.location.is_none() {
            self.testing.worktrees.get_mut(&scope).unwrap().error =
                "This test has no importable source definition for debugging".into();
            return;
        }
        let id = case.id.clone();
        let Some(root) = self.root_of(scope) else {
            return;
        };
        self.point_at(scope);
        match Python::read(&root) {
            Ok(adapter) => self.start_debugging(
                scope,
                pm_dap::Scenario {
                    label: format!("Test: {id}"),
                    kind: "debugpy".into(),
                    adapter: pm_dap::Adapter::find("debugpy"),
                    request: pm_dap::Request::Launch,
                    before: None,
                    config: adapter.debug_arguments(&root, &id),
                    source: None,
                },
            ),
            Err(error) => self.testing.worktrees.get_mut(&scope).unwrap().error = error,
        }
    }

    /// Builds the explorer grouped by project and each independently scoped worktree.
    pub(super) fn test_content(&self, theme: &Theme) -> Div<Message> {
        let groups = self
            .worktrees()
            .into_iter()
            .map(|(scope, root)| {
                let project = self
                    .open
                    .get(scope.project())
                    .map_or("", |project| project.name());
                (
                    scope,
                    format!(
                        "{project} / {} · {}",
                        self.worktree_name(scope),
                        root.display()
                    ),
                )
            })
            .collect::<Vec<_>>();
        crate::testing::explorer(theme, &self.testing, &groups, |scope| {
            self.test_coverage_stale(scope)
        })
    }

    /// Detects saved and unsaved edits before coverage can be drawn as current.
    fn test_coverage_stale(&self, scope: Scope) -> bool {
        self.testing
            .worktrees
            .get(&scope)
            .and_then(|tree| tree.coverage.as_ref())
            .is_none_or(|coverage| {
                coverage.stale
                    || coverage.files.iter().any(|file| !file.current())
                    || self
                        .open_files()
                        .iter()
                        .any(|(held, _, file)| *held == scope && self.editor.is_dirty(*file))
            })
    }

    /// Current coverage marks for the exact file and worktree of an editor pane.
    pub(super) fn coverage_of(&self, file: crate::editor::FileId) -> Vec<(usize, bool)> {
        let Some(scope) = self.editor.scope_of(file) else {
            return Vec::new();
        };
        if self.test_coverage_stale(scope) {
            return Vec::new();
        }
        let Some(path) = self
            .editor
            .path(file)
            .and_then(|path| path.canonicalize().ok())
        else {
            return Vec::new();
        };
        self.testing
            .worktrees
            .get(&scope)
            .and_then(|tree| tree.coverage.as_ref())
            .and_then(|coverage| coverage.files.iter().find(|file| file.path == path))
            .map_or_else(Vec::new, |file| {
                file.lines
                    .iter()
                    .map(|(line, count)| (*line, *count > 0))
                    .collect()
            })
    }

    /// Scrolls the ordinary explorer pane by whole visible rows.
    pub(super) fn scroll_tests(&mut self, rows: isize) -> bool {
        let over = self
            .pointer
            .and_then(|pointer| self.geometry.pane_at(pointer))
            .and_then(|pane| self.panes.pane(pane))
            .is_some_and(|pane| pane.active(self.scope()) == Some(Item::Tool(Tool::Tests)));
        if over {
            self.testing.scroll = self.testing.scroll.saturating_add_signed(rows);
        }
        over
    }

    /// Refreshes opened scopes and removes artifacts for worktrees that left the window.
    pub(super) fn maintain_tests(&mut self) {
        let scopes = self.scopes();
        self.testing
            .worktrees
            .retain(|scope, _| scopes.contains(scope));
        if self.tool_visible(Tool::Tests) {
            for scope in &scopes {
                self.testing.worktrees.entry(*scope).or_insert_with(|| {
                    let mut tree = Worktree::default();
                    tree.refresh = true;
                    tree
                });
            }
        }
        for scope in scopes {
            self.refresh_tests(scope);
        }
    }
}
