//! The one seam for starting, stopping and observing worktree tasks.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use pm_core::{ProjectId, Scope, Task};

use crate::terminal::{Shell, ShellId, Terminals};

/// A task run's identity within a window.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RunId(u64);

/// How a task run ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The command exited successfully.
    Succeeded,
    /// The command exited with a nonzero code.
    Failed(u32),
    /// A signal ended the command.
    Killed(String),
    /// A caller stopped the command.
    Stopped,
}

/// Whether a task should take the terminal view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shown {
    /// Show the run immediately.
    Front,
    /// Leave the view where it is.
    Quiet,
}

/// One completed run, handed to the window once.
#[derive(Clone, Debug)]
pub struct Finished {
    /// The run's identity.
    pub run: RunId,
    /// The worktree the run belongs to.
    pub scope: Scope,
    /// The task's label.
    pub label: String,
    /// How it ended.
    pub outcome: Outcome,
    /// Whether it was brought to the front.
    pub shown: Shown,
}

/// A run and its retained shell.
struct Running {
    /// Worktree in which it runs.
    scope: Scope,
    /// Its label.
    label: String,
    /// Its terminal list identity.
    shell_id: ShellId,
    /// The child and output.
    shell: Shell,
    /// Whether it took the terminal view.
    shown: Shown,
    /// Its final result, if any.
    outcome: Option<Outcome>,
}

/// All task runs and their outcomes, scoped to worktrees.
#[derive(Default)]
pub struct Tasks {
    /// Runs by their identity.
    runs: BTreeMap<RunId, Running>,
    /// Last task started in each worktree.
    last: BTreeMap<Scope, Task>,
    /// Runs finished since the last collection.
    finished: Vec<Finished>,
    /// Identity for the next run.
    next: u64,
}

impl Tasks {
    /// The retained output tail of a task, including its final terminal writes.
    pub fn tail(&self, run: RunId, lines: usize) -> String {
        self.runs
            .get(&run)
            .map_or_else(String::new, |run| run.shell.borrow().tail(lines))
    }

    /// Opens a retained task shell through the terminal list.
    pub fn show(&self, terminals: &mut Terminals, run: RunId) -> Option<Scope> {
        let held = self.runs.get(&run)?;
        terminals.activate(held.scope, held.shell_id);
        Some(held.scope)
    }

    /// Starts `task` in `scope`, stopping an earlier run of its label first.
    pub fn run(
        &mut self,
        terminals: &mut Terminals,
        scope: Scope,
        root: &Path,
        task: &Task,
        env: &[(String, String)],
        shown: Shown,
    ) -> Result<RunId, String> {
        if task.cwd.as_ref().is_some_and(|cwd| {
            cwd.components()
                .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        }) {
            return Err("Task working directory must be inside its worktree".to_owned());
        }
        let previous = self
            .runs
            .iter()
            .find(|(_, run)| run.scope == scope && run.label == task.label && run.outcome.is_none())
            .map(|(id, run)| (*id, run.shell_id));
        if let Some((old, _)) = previous {
            self.stop(old);
        }
        let cwd = task
            .cwd
            .as_ref()
            .map_or_else(|| root.to_path_buf(), |relative| root.join(relative));
        let mut environment = env.to_vec();
        environment.extend(task.env.iter().cloned());
        let shell_id = terminals
            .run_task(
                scope,
                &cwd,
                task,
                &environment,
                previous.map(|(_, shell)| shell),
                shown == Shown::Front,
            )
            .map_err(|error| error.to_string())?;
        let shell = terminals
            .task_shell(scope, shell_id)
            .ok_or("Task shell disappeared")?;
        let id = RunId(self.next);
        self.next += 1;
        self.runs.insert(
            id,
            Running {
                scope,
                label: task.label.clone(),
                shell_id,
                shell,
                shown,
                outcome: None,
            },
        );
        self.last.insert(scope, task.clone());
        Ok(id)
    }

    /// Stops a running task and records that a caller stopped it.
    pub fn stop(&mut self, run: RunId) {
        if let Some(running) = self.runs.get_mut(&run)
            && running.outcome.is_none()
        {
            running.shell.borrow_mut().kill();
            Self::finish(&mut self.finished, run, running, Outcome::Stopped);
        }
    }

    /// The final result of `run`, or nothing while it is running.
    pub fn outcome(&self, run: RunId) -> Option<Outcome> {
        self.runs.get(&run)?.outcome.clone()
    }

    /// Runs completed since this was last called.
    pub fn take_finished(&mut self) -> Vec<Finished> {
        std::mem::take(&mut self.finished)
    }

    /// The last task started in `scope`.
    pub fn last(&self, scope: Scope) -> Option<Task> {
        self.last.get(&scope).cloned()
    }

    /// Whether a task of `label` is still running in `scope`.
    pub fn running(&self, scope: Scope, label: &str) -> bool {
        self.runs
            .values()
            .any(|run| run.scope == scope && run.label == label && run.outcome.is_none())
    }

    /// The run shown in the terminal view, if it is a task.
    pub fn shown(&self, scope: Scope, terminals: &Terminals) -> Option<RunId> {
        let shell = terminals.active_task(scope)?;
        self.runs
            .iter()
            .find(|(_, run)| run.scope == scope && run.shell_id == shell && run.outcome.is_none())
            .map(|(id, _)| *id)
    }

    /// The sole running task in `scope`, if there is exactly one.
    pub fn only_running(&self, scope: Scope) -> Option<RunId> {
        let mut runs = self
            .runs
            .iter()
            .filter(|(_, run)| run.scope == scope && run.outcome.is_none())
            .map(|(id, _)| *id);
        let first = runs.next()?;
        runs.next().is_none().then_some(first)
    }

    /// Stops a task whose listed terminal shell is being closed.
    pub fn stop_shell(&mut self, scope: Scope, shell: ShellId) {
        if let Some(id) = self
            .runs
            .iter()
            .find(|(_, run)| run.scope == scope && run.shell_id == shell && run.outcome.is_none())
            .map(|(id, _)| *id)
        {
            self.stop(id);
        }
    }

    /// Stops task shells removed from a worktree's terminal list.
    pub fn stop_shells_except(&mut self, scope: Scope, kept: Option<ShellId>) {
        let ids = self
            .runs
            .iter()
            .filter(|(_, run)| {
                run.scope == scope && Some(run.shell_id) != kept && run.outcome.is_none()
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in ids {
            self.stop(id);
        }
    }

    /// Observes child exits after terminal output has been pumped.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        for (id, run) in &mut self.runs {
            if run.outcome.is_some() {
                continue;
            }
            let mut shell = run.shell.borrow_mut();
            if shell.is_running() {
                continue;
            }
            shell.pump();
            let outcome = match (shell.exit_signal(), shell.exit_code()) {
                (Some(signal), _) => Outcome::Killed(signal),
                (_, Some(0)) => Outcome::Succeeded,
                (_, Some(code)) => Outcome::Failed(code),
                _ => Outcome::Killed("unknown".to_owned()),
            };
            drop(shell);
            Self::finish(&mut self.finished, *id, run, outcome);
            changed = true;
        }
        changed
    }

    /// Stops and forgets every run of a project leaving the window.
    pub fn forget(&mut self, project: ProjectId) {
        for run in self
            .runs
            .values_mut()
            .filter(|run| run.scope.project() == project && run.outcome.is_none())
        {
            run.shell.borrow_mut().kill();
        }
        self.runs.retain(|_, run| run.scope.project() != project);
        self.last.retain(|scope, _| scope.project() != project);
        self.finished.retain(|run| run.scope.project() != project);
    }

    /// Stops and forgets every run of a worktree being removed.
    pub fn forget_scope(&mut self, scope: Scope) {
        for run in self
            .runs
            .values_mut()
            .filter(|run| run.scope == scope && run.outcome.is_none())
        {
            run.shell.borrow_mut().kill();
        }
        self.runs.retain(|_, run| run.scope != scope);
        self.last.remove(&scope);
        self.finished.retain(|run| run.scope != scope);
    }

    /// Records a run's result once.
    fn finish(finished: &mut Vec<Finished>, id: RunId, run: &mut Running, outcome: Outcome) {
        run.outcome = Some(outcome.clone());
        finished.push(Finished {
            run: id,
            scope: run.scope,
            label: run.label.clone(),
            outcome,
            shown: run.shown,
        });
    }
}
