//! Test discoveries, runs and coverage owned independently by each worktree.

use crate::tasks::{Outcome, RunId};
use pm_core::Scope;
use pm_core::testing::{Case, Coverage, SourceRevision, Status};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A test explorer command, with indices resolved in its originating scope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    /// Refresh the framework's discovery.
    Refresh,
    /// Run every discovered test.
    All,
    /// Run the suite containing this case.
    Suite(usize),
    /// Run one case.
    Case(usize),
    /// Debug one case through the existing debugger.
    Debug(usize),
    /// Stop the current discovery or test task.
    Cancel,
    /// Show a case's output and navigate to its failure or definition.
    Source(usize),
    /// Collect coverage while running the full suite.
    Cover,
    /// Import coverage.lcov from the worktree root.
    Import,
    /// Remove every coverage decoration in the worktree.
    Clear,
    /// Navigate to a covered file's first uncovered line.
    CoverageSource(usize),
    /// Show a retained run and its cases.
    History(usize),
    /// Open the retained run's complete terminal output.
    Output,
}

/// One retained run, including interrupted results and the source revision it tested.
pub struct TestRun {
    /// Task identity for cancellation and terminal output.
    pub task: RunId,
    /// Unique journal directory.
    pub directory: PathBuf,
    /// Cases and their results for this run only.
    pub cases: Vec<Case>,
    /// Saved source revision captured before the task started.
    pub revision: SourceRevision,
    /// Final task outcome, including infrastructure errors.
    pub outcome: Option<Outcome>,
    /// Whether this task requested coverage collection.
    pub coverage: bool,
    /// Source epoch captured before the task started.
    pub epoch: u64,
    /// Full terminal tail for failed and interrupted tasks.
    pub output: String,
}

impl TestRun {
    /// Applies complete journal records and preserves the active case's interrupted output.
    pub fn read(&mut self) {
        let text =
            std::fs::read_to_string(self.directory.join("results.jsonl")).unwrap_or_default();
        for line in text.lines() {
            let Ok(case) = serde_json::from_str::<Case>(line) else {
                continue;
            };
            if let Some(held) = self.cases.iter_mut().find(|held| held.id == case.id) {
                *held = case;
            } else {
                self.cases.push(case);
            }
        }
        if self.outcome.is_some() {
            for case in &mut self.cases {
                if matches!(case.status, Status::Queued | Status::Running) {
                    if case.status == Status::Running {
                        case.output.push_str(
                            &std::fs::read_to_string(self.directory.join("active-output.txt"))
                                .unwrap_or_default(),
                        );
                        case.duration = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_or(0.0, |now| (now.as_secs_f64() - case.started).max(0.0));
                    }
                    case.status = if self.outcome == Some(Outcome::Stopped)
                        || matches!(self.outcome, Some(Outcome::Killed(_)))
                    {
                        Status::Cancelled
                    } else {
                        Status::Failed
                    };
                    if case.output.is_empty() {
                        case.output.clone_from(&self.output);
                    }
                }
            }
        }
    }
}

/// A discovery task in flight, tagged with the source epoch it read.
pub struct Pending {
    /// Existing task runner identity.
    pub task: RunId,
    /// Destination of discovery output.
    pub directory: PathBuf,
    /// Source epoch before discovery.
    pub epoch: u64,
}

/// Every test artifact belonging to one project and worktree scope.
#[derive(Default)]
pub struct Worktree {
    /// Current discovered identities and source definitions.
    pub cases: Vec<Case>,
    /// Independent results of each run.
    pub runs: Vec<TestRun>,
    /// Discovery currently running through Tasks.
    pub discovery: Option<Pending>,
    /// Discovery should refresh after source changes.
    pub refresh: bool,
    /// Latest source-change epoch.
    pub epoch: u64,
    /// Retained run currently inspected.
    pub selected_run: Option<usize>,
    /// Case output currently inspected.
    pub selected_case: Option<usize>,
    /// Whether the explorer is inspecting a historical subset instead of the inventory.
    pub history: bool,
    /// Latest coverage and its revision validity.
    pub coverage: Option<Coverage>,
    /// Discovery or coverage infrastructure error.
    pub error: String,
}

impl Worktree {
    /// Cases of a historical run while inspecting history, or the complete current inventory.
    pub fn shown(&self) -> &[Case] {
        self.selected_run
            .filter(|_| self.history)
            .and_then(|index| self.runs.get(index))
            .map_or(&self.cases, |run| &run.cases)
    }

    /// The current task, with no second launcher or independent exit detection.
    pub fn active(&self) -> Option<RunId> {
        self.discovery
            .as_ref()
            .map(|pending| pending.task)
            .or_else(|| {
                self.runs
                    .iter()
                    .rev()
                    .find(|run| run.outcome.is_none())
                    .map(|run| run.task)
            })
    }

    /// Invalidates source-bound coverage and schedules rediscovery after an edit.
    pub fn changed(&mut self) {
        self.epoch += 1;
        self.refresh = true;
        if let Some(coverage) = &mut self.coverage {
            coverage.invalidate();
        }
    }
}

/// All explorer data keyed by full worktree scope.
#[derive(Default)]
pub struct Store {
    /// Discoveries, results and coverage for each open worktree.
    pub worktrees: BTreeMap<Scope, Worktree>,
    /// Vertical row offset shared by views of the explorer.
    pub scroll: usize,
    /// Unique task artifact counter in this editor process.
    pub next: u64,
}

impl Drop for Worktree {
    /// Removes private journals after tasks are stopped and the worktree leaves the window.
    fn drop(&mut self) {
        if let Some(discovery) = &self.discovery {
            let _ = std::fs::remove_dir_all(&discovery.directory);
        }
        for run in &self.runs {
            let _ = std::fs::remove_dir_all(&run.directory);
        }
    }
}
