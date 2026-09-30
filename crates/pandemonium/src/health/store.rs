//! Check results, scheduling and feedback budgets keyed by worktree.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use pm_core::{Scope, Summary, Task};

use super::Health;
use crate::tasks::{Outcome, RunId};

/// A task definition and the most recent check result.
pub struct Check {
    /// The command as it was run.
    pub task: Task,
    /// Its task runner identity, once started.
    pub run: Option<RunId>,
    /// Its final status, once finished.
    pub outcome: Option<Outcome>,
    /// The last forty lines, or the reason it could not start.
    pub tail: String,
}

/// Check history and feedback state for one worktree.
#[derive(Default)]
pub struct Worktree {
    /// Check tasks in definition order.
    pub checks: Vec<Check>,
    /// Whether a run is queued or executing.
    pub running: bool,
    /// Whether this run owns one of the two execution slots.
    pub active: bool,
    /// Whether this run followed an agent turn.
    pub automatic: bool,
    /// Drift at the start of the most recent run.
    pub summary: Option<Summary>,
    /// Current diagnostic error count.
    pub errors: usize,
    /// Whether any language server supplies diagnostics.
    pub servers: bool,
    /// Automatic prompts sent since the last success or reader prompt.
    pub retries: usize,
    /// Whether the transcript has already explained the exhausted budget.
    pub stopped: bool,
}

impl Check {
    /// Keeps a definition ready for its first check run.
    fn new(task: Task) -> Self {
        Self {
            task,
            run: None,
            outcome: None,
            tail: String::new(),
        }
    }
}

impl Worktree {
    /// Computes health from checks and diagnostics, giving running precedence.
    pub fn health(&self) -> Health {
        if self.running {
            Health::Running
        } else if self.errors > 0
            || self.checks.iter().any(|check| {
                check
                    .outcome
                    .as_ref()
                    .is_some_and(|outcome| *outcome != Outcome::Succeeded)
            })
        {
            Health::Failing
        } else if self
            .checks
            .iter()
            .all(|check| check.outcome == Some(Outcome::Succeeded))
            && (!self.checks.is_empty() || self.servers)
        {
            Health::Passing
        } else {
            Health::Unknown
        }
    }

    /// Describes failing commands, diagnostic errors and any retry limit.
    pub fn detail(&self) -> String {
        let mut detail = self
            .checks
            .iter()
            .filter_map(|check| {
                let result = match check.outcome.as_ref()? {
                    Outcome::Succeeded => return None,
                    Outcome::Failed(code) => format!("failed (exit {code})"),
                    Outcome::Killed(signal) => format!("killed ({signal})"),
                    Outcome::Stopped => "stopped".to_owned(),
                };
                Some(format!("{} {result}", check.task.label))
            })
            .collect::<Vec<_>>();
        if self.errors > 0 {
            detail.push(format!("{} errors", self.errors));
        }
        if self.stopped {
            detail.push(format!("Stopped retrying after {} attempts", self.retries));
        }
        if detail.is_empty() {
            detail.push(
                match self.health() {
                    Health::Unknown => "Not checked",
                    Health::Running => "Checks running",
                    Health::Passing => "Checks passing",
                    Health::Failing => "Checks failing",
                }
                .to_owned(),
            );
        }
        detail.join(" · ")
    }
}

/// Health of every worktree and the queue sharing two execution slots.
#[derive(Default)]
pub struct Checks {
    /// Results and retry counts by project and session.
    pub worktrees: BTreeMap<Scope, Worktree>,
    /// Worktrees waiting for an execution slot.
    pub queue: VecDeque<Scope>,
    /// Agent turns waiting for freshly read drift.
    pub turns: BTreeSet<Scope>,
}

impl Checks {
    /// Initializes unchecked task definitions before diagnostic health is computed.
    pub fn observe(&mut self, scope: Scope, tasks: Vec<Task>) {
        self.worktrees.entry(scope).or_insert_with(|| Worktree {
            checks: tasks
                .into_iter()
                .filter(|task| task.check)
                .map(Check::new)
                .collect(),
            ..Worktree::default()
        });
    }

    /// The computed state of a worktree, unknown until observed.
    pub fn health(&self, scope: Scope) -> Health {
        self.worktrees
            .get(&scope)
            .map_or(Health::Unknown, Worktree::health)
    }

    /// The detail displayed beside the worktree's badge.
    pub fn detail(&self, scope: Scope) -> String {
        self.worktrees
            .get(&scope)
            .map_or_else(|| "Not checked".to_owned(), Worktree::detail)
    }

    /// Queues a fresh run while preserving diagnostic and feedback state.
    pub fn begin(
        &mut self,
        scope: Scope,
        tasks: Vec<Task>,
        automatic: bool,
        summary: Option<Summary>,
    ) -> Vec<RunId> {
        self.queue.retain(|queued| *queued != scope);
        let held = self.worktrees.entry(scope).or_default();
        let previous = held.checks.iter().filter_map(|check| check.run).collect();
        held.checks = tasks.into_iter().map(Check::new).collect();
        held.running = true;
        held.active = false;
        held.automatic = automatic;
        held.summary = summary;
        self.queue.push_back(scope);
        previous
    }

    /// Resets the feedback budget after a reader prompt or passing run.
    pub fn reset(&mut self, scope: Scope) {
        if let Some(held) = self.worktrees.get_mut(&scope) {
            held.retries = 0;
            held.stopped = false;
        }
    }

    /// Removes health and queued work when a worktree leaves the window.
    pub fn forget(&mut self, scope: Scope) {
        self.worktrees.remove(&scope);
        self.queue.retain(|queued| *queued != scope);
        self.turns.remove(&scope);
    }
}
