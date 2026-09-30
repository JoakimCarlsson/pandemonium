//! The seam joining worktree checks, task exits and agent feedback.

use pm_core::Scope;

use crate::app::App;
use crate::health::Health;
use crate::panel::PanelView;
use crate::tasks::{Outcome, RunId, Shown};

impl App {
    /// Queues the worktree's health tasks, cancelling its previous check run.
    pub(super) fn run_checks(&mut self, scope: Scope, automatic: bool) {
        if self.root_of(scope).is_none() {
            return;
        }
        self.refresh_health_diagnostics();
        let tasks = self
            .available_tasks(scope)
            .into_iter()
            .filter(|task| task.check)
            .collect();
        let summary = scope
            .session()
            .and_then(|session| self.sessions.get(session))
            .map(pm_core::Session::summary);
        for run in self.checks.begin(scope, tasks, automatic, summary) {
            self.tasks.stop(run);
        }
        self.hear_finished_tasks();
    }

    /// Waits for a fresh drift reading after each fully completed agent turn.
    pub(super) fn hear_health_turns(&mut self) {
        let turns = self.agents.take_turns();
        if !turns.is_empty() {
            self.checks
                .turns
                .extend(turns.into_iter().filter(|scope| scope.session().is_some()));
            self.reread_drift_later();
        }
        self.advance_checks();
    }

    /// Runs only completed turns whose newly read drift differs from the last run.
    pub(super) fn check_changed_turns(&mut self) {
        for scope in std::mem::take(&mut self.checks.turns) {
            let Some(session) = scope.session().and_then(|id| self.sessions.get(id)) else {
                continue;
            };
            let summary = session.summary();
            let previous = self
                .checks
                .worktrees
                .get(&scope)
                .and_then(|held| held.summary);
            if previous != Some(summary)
                && (previous.is_some() || summary != pm_core::Summary::default())
            {
                self.run_checks(scope, true);
            } else if self.preferences.health_feedback
                && self.checks.worktrees.get(&scope).is_some_and(|held| {
                    held.automatic
                        && !held.running
                        && held.retries >= self.preferences.health_retries
                })
            {
                self.finish_checks(scope);
            }
        }
    }

    /// Whether the worktree's agent is currently inside a turn.
    fn checking_busy(&self, scope: Scope) -> bool {
        self.agents
            .iter()
            .any(|talk| talk.scope() == scope && talk.is_busy())
    }

    /// Takes a task result only if it belongs to the current check run.
    pub(super) fn take_check_result(&mut self, run: RunId, outcome: &Outcome) {
        let tail = self.tasks.tail(run, 40);
        for held in self.checks.worktrees.values_mut() {
            if let Some(check) = held.checks.iter_mut().find(|check| check.run == Some(run)) {
                check.outcome = Some(outcome.clone());
                check.tail = tail;
                break;
            }
        }
    }

    /// Advances ordered tasks in at most two worktrees through the existing runner.
    pub(super) fn advance_checks(&mut self) {
        loop {
            let active = self
                .checks
                .worktrees
                .iter()
                .filter(|(_, held)| held.active)
                .map(|(scope, _)| *scope)
                .collect::<Vec<_>>();
            for scope in active {
                let held = &self.checks.worktrees[&scope];
                if held
                    .checks
                    .iter()
                    .any(|check| check.run.is_some() && check.outcome.is_none())
                {
                    continue;
                }
                if self.checking_busy(scope) {
                    continue;
                }
                let next = held.checks.iter().position(|check| check.outcome.is_none());
                if let Some(index) = next {
                    let task = held.checks[index].task.clone();
                    let Some(root) = self.root_of(scope) else {
                        self.checks.forget(scope);
                        continue;
                    };
                    let env = self.worktree_env(scope);
                    let started = self.tasks.run(
                        &mut self.terminals,
                        scope,
                        &root,
                        &task,
                        &env,
                        Shown::Quiet,
                    );
                    let check = &mut self.checks.worktrees.get_mut(&scope).unwrap().checks[index];
                    match started {
                        Ok(run) => check.run = Some(run),
                        Err(error) => {
                            check.outcome = Some(Outcome::Killed("could not start".to_owned()));
                            check.tail = error;
                        }
                    }
                } else {
                    let held = self.checks.worktrees.get_mut(&scope).unwrap();
                    held.running = false;
                    held.active = false;
                    self.finish_checks(scope);
                }
            }
            let count = self
                .checks
                .worktrees
                .values()
                .filter(|held| held.active)
                .count();
            let eligible = self
                .checks
                .queue
                .iter()
                .position(|scope| !self.checking_busy(*scope));
            if count < 2
                && let Some(index) = eligible
            {
                let scope = self.checks.queue.remove(index).unwrap();
                if let Some(held) = self.checks.worktrees.get_mut(&scope) {
                    held.active = true;
                }
                continue;
            }
            let ready = self.checks.worktrees.iter().any(|(scope, held)| {
                held.active
                    && !self.checking_busy(*scope)
                    && !held
                        .checks
                        .iter()
                        .any(|check| check.run.is_some() && check.outcome.is_none())
            });
            if !ready {
                break;
            }
        }
    }

    /// Updates diagnostic counts without running commands or failing warnings.
    pub(super) fn refresh_health_diagnostics(&mut self) {
        let scopes = self
            .open
            .iter()
            .flat_map(|project| {
                std::iter::once(Scope::checkout(project.id())).chain(
                    self.sessions
                        .of(project.id())
                        .map(|session| Scope::of(project.id(), session.id())),
                )
            })
            .collect::<Vec<_>>();
        for scope in scopes {
            let Some(root) = self.root_of(scope) else {
                continue;
            };
            if !self.checks.worktrees.contains_key(&scope) {
                let tasks = self.available_tasks(scope);
                self.checks.observe(scope, tasks);
            }
            let servers = self.editor.servers_over(&root);
            let held = self.checks.worktrees.entry(scope).or_default();
            held.servers = !servers.is_empty();
            held.errors = servers.iter().map(|server| server.errors()).sum();
        }
        if self
            .picker
            .as_ref()
            .is_some_and(|picker| picker.kind() == crate::picker::Kind::Sessions)
        {
            let rows = self.session_rows();
            if let Some(picker) = self.picker.as_mut() {
                picker.refill_preserving_selection(rows);
            }
        }
    }

    /// Finishes a run and optionally sends bounded failure feedback as a transcript turn.
    fn finish_checks(&mut self, scope: Scope) {
        self.refresh_health_diagnostics();
        if self.checks.health(scope) == Health::Passing {
            self.checks.reset(scope);
            return;
        }
        let held = &self.checks.worktrees[&scope];
        if self.checks.health(scope) != Health::Failing
            || !held.automatic
            || !self.preferences.health_feedback
        {
            return;
        }
        let Some(talk_id) = scope
            .session()
            .and_then(|session| self.agents.of_session(session))
        else {
            return;
        };
        let Some(talk) = self.agents.get(talk_id) else {
            return;
        };
        if talk.is_busy() || !talk.is_running() || !talk.is_ready() || !talk.asks().is_empty() {
            return;
        }
        if held.retries >= self.preferences.health_retries {
            if !held.stopped {
                let note = format!("Stopped retrying after {} attempts", held.retries);
                self.agents.get_mut(talk_id).unwrap().note(note);
                self.checks.worktrees.get_mut(&scope).unwrap().stopped = true;
            }
            return;
        }
        let mut prompt = String::from("The worktree's health checks failed.\n\n");
        for check in &held.checks {
            if let Some(outcome) = &check.outcome
                && *outcome != Outcome::Succeeded
            {
                let exit = match outcome {
                    Outcome::Failed(code) => format!("exit {code}"),
                    Outcome::Killed(signal) => format!("killed: {signal}"),
                    Outcome::Stopped => "stopped".to_owned(),
                    Outcome::Succeeded => unreachable!(),
                };
                let fence = "`".repeat(
                    check
                        .tail
                        .lines()
                        .map(|line| line.chars().take_while(|ch| *ch == '`').count() + 1)
                        .max()
                        .unwrap_or(3)
                        .max(3),
                );
                prompt.push_str(&format!(
                    "{}: {} ({exit})\n{fence}\n{}\n{fence}\n\n",
                    check.task.label, check.task.command, check.tail
                ));
            }
        }
        if let Some(root) = self.root_of(scope) {
            let mut errors = self
                .editor
                .servers_over(&root)
                .iter()
                .flat_map(|server| server.error_diagnostics())
                .collect::<Vec<_>>();
            errors.sort_by(|(a, x), (b, y)| {
                (a, x.range.start.line, &x.message).cmp(&(b, y.range.start.line, &y.message))
            });
            errors
                .dedup_by(|(a, x), (b, y)| a == b && x.range == y.range && x.message == y.message);
            for (path, diagnostic) in errors.into_iter().take(20) {
                prompt.push_str(&format!(
                    "{}:{}: {}\n",
                    path.strip_prefix(&root).unwrap_or(&path).display(),
                    diagnostic.range.start.line + 1,
                    diagnostic.message
                ));
            }
        }
        prompt.push_str("\nFix these and stop when the checks pass.");
        self.agents.get_mut(talk_id).unwrap().send_text(&prompt);
        self.checks.worktrees.get_mut(&scope).unwrap().retries += 1;
    }

    /// Opens the first failing check's retained task output, or the first available run.
    pub(super) fn show_check_output(&mut self, scope: Scope) {
        let Some(held) = self.checks.worktrees.get(&scope) else {
            return;
        };
        let run = held
            .checks
            .iter()
            .find(|check| {
                check
                    .outcome
                    .as_ref()
                    .is_some_and(|outcome| *outcome != Outcome::Succeeded)
                    && check.run.is_some()
            })
            .and_then(|check| check.run)
            .or_else(|| held.checks.iter().find_map(|check| check.run));
        if let Some(run) = run {
            self.point_at(scope);
            self.tasks.show(&mut self.terminals, run);
            self.show_panel(PanelView::Terminal);
        }
    }
}
