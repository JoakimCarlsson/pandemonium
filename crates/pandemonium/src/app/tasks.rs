//! Task picker, actions and notices for completed runs.

use pm_core::{Scope, Task, TaskSource};

use crate::app::App;
use crate::keymap::Action;
use crate::message::Message;
use crate::panel::PanelView;
use crate::picker::{Choice, Kind, Row};
use crate::tasks::{Outcome, RunId, Shown};

impl App {
    /// Carries out task actions from the keymap or command palette.
    pub(super) fn task_action(&mut self, action: Action) -> bool {
        match action {
            Action::TaskRun => self.open_picker(Kind::Tasks),
            Action::TaskRerun => {
                if let Some(scope) = self.scope() {
                    if let Some(last) = self.tasks.last(scope) {
                        if let Some(task) = self
                            .available_tasks(scope)
                            .into_iter()
                            .find(|task| task.label == last.label)
                        {
                            self.run_task(scope, &task, Shown::Front);
                        } else {
                            self.open_picker(Kind::Tasks);
                        }
                    } else {
                        self.open_picker(Kind::Tasks);
                    }
                }
            }
            Action::TaskStop => {
                if let Some(scope) = self.scope()
                    && let Some(run) = self
                        .tasks
                        .shown(scope, &self.terminals)
                        .or_else(|| self.tasks.only_running(scope))
                {
                    self.tasks.stop(run);
                    self.hear_finished_tasks();
                }
            }
            _ => return false,
        }
        true
    }

    /// Rows of tasks detected or defined by the worktree in front.
    pub(super) fn task_rows(&mut self) -> Vec<Row> {
        if self.scope().is_some_and(|scope| self.is_remote(scope)) {
            return self.unsupported_row("Tasks");
        }
        let Some(scope) = self.scope() else {
            return Vec::new();
        };
        self.available_tasks(scope)
            .into_iter()
            .map(|task| {
                let source = match task.source {
                    TaskSource::File => ".pandemonium/tasks.yaml",
                    TaskSource::Detected("cargo") => "Cargo.toml",
                    TaskSource::Detected("npm") => "package.json",
                    TaskSource::Detected("make") => "Makefile",
                    TaskSource::Detected("go") => "go.mod",
                    TaskSource::Detected(other) => other,
                };
                let detail = if self.tasks.running(scope, &task.label) {
                    format!("{} · {source} · running", task.command)
                } else {
                    format!("{} · {source}", task.command)
                };
                Row {
                    section: None,
                    label: task.label.clone(),
                    detail,
                    choice: Choice::Task(scope, Box::new(task)),
                    enabled: true,
                }
            })
            .collect()
    }

    /// Reads tasks and reports an invalid project tasks file once.
    pub(super) fn available_tasks(&mut self, scope: Scope) -> Vec<Task> {
        let Some(root) = self.root_of(scope) else {
            return Vec::new();
        };
        if self.is_remote(scope) {
            return Vec::new();
        }
        let (tasks, error) = pm_core::tasks_checked(&root);
        if let Some(error) = error
            && self.task_errors.insert(root.path)
        {
            self.notices.trouble(error, None);
        }
        tasks
    }

    /// Runs a task through the one task seam and optionally shows its shell.
    pub(super) fn run_task(&mut self, scope: Scope, task: &Task, shown: Shown) -> Option<RunId> {
        if self.refuse_remote(scope, "Tasks") {
            return None;
        }
        let root = self.root_of(scope)?;
        let env = self.worktree_env(scope);
        let started = self
            .tasks
            .run(&mut self.terminals, scope, &root, task, &env, shown);
        self.hear_finished_tasks();
        match started {
            Ok(run) => {
                if shown == Shown::Front && self.scope() == Some(scope) {
                    self.show_panel(PanelView::Terminal);
                }
                Some(run)
            }
            Err(error) => {
                self.say_trouble("Task could not start", &error);
                None
            }
        }
    }

    /// Turns task exits into foreground notices and starts waiting debuggers.
    pub(super) fn hear_finished_tasks(&mut self) {
        for finished in self.tasks.take_finished() {
            self.take_check_result(finished.run, &finished.outcome);
            if let Some((scope, scenario)) = self.pending_debug.remove(&finished.run) {
                if self.tasks.outcome(finished.run) == Some(Outcome::Succeeded) {
                    self.start_debug_adapter(scope, scenario);
                } else {
                    self.notices.trouble(
                        format!(
                            "Debugging did not start: `{}` {}",
                            finished.label,
                            result(&finished.outcome)
                        ),
                        None,
                    );
                }
                continue;
            }
            if finished.shown == Shown::Quiet
                || finished.outcome == Outcome::Succeeded
                || finished.outcome == Outcome::Stopped
            {
                continue;
            }
            let place = if self.scope() == Some(finished.scope) {
                String::new()
            } else {
                format!(" in {}", self.worktree_name(finished.scope))
            };
            self.notices.trouble(
                format!("`{}` {}{place}", finished.label, result(&finished.outcome)),
                Some(Message::ShowPanelView(PanelView::Terminal)),
            );
        }
        self.advance_checks();
    }
}

/// A short description of an unsuccessful task result.
fn result(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Failed(code) => format!("failed (exit {code})"),
        Outcome::Killed(signal) => format!("killed ({signal})"),
        Outcome::Stopped => "stopped".to_owned(),
        Outcome::Succeeded => "succeeded".to_owned(),
    }
}
