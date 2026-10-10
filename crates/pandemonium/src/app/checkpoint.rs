//! The serialized seam between agent boundaries, durable turn history and rewind.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use pm_core::{Checkpoint, CheckpointStep, Scope};

use crate::agent::TalkId;
use crate::app::App;
use crate::message::Message;
use crate::panes::{Item, TurnSpan};
use crate::picker::{Choice, Kind, Row};
use crate::prompt::{Answer, Prompt};
use crate::review::TurnDiff;

/// Checkpoint state whose durable source is git's worktree-local refs.
#[derive(Default)]
pub(super) struct Checkpointing {
    /// Turns belonging to worktrees read in this window.
    pub(super) turns: BTreeMap<Scope, Vec<Checkpoint>>,
    /// Provenance for completed and in-progress turns.
    steps: BTreeMap<Scope, Vec<CheckpointStep>>,
    /// Conversations waiting on their first snapshot.
    starting: BTreeSet<TalkId>,
    /// Current turn and prompt per conversation.
    active: BTreeMap<TalkId, (Scope, u64, String)>,
    /// Open comparisons.
    pub(super) diffs: BTreeMap<(Scope, TurnSpan), TurnDiff>,
    /// Worktrees awaiting confirmation or execution of a rewind.
    rewinding: BTreeSet<Scope>,
}

/// One filesystem operation queued alongside review work.
#[derive(Clone)]
pub(super) enum CheckpointWork {
    /// Capture a prompt's baseline before delivering it.
    Begin(TalkId, String),
    /// Capture a completed file-changing tool notification.
    Step(u64, Box<pm_acp::ToolCall>),
    /// Persist the end of a turn.
    End(u64, String),
    /// List persisted turns after a launch.
    List(Option<u64>),
    /// Read a comparison of two saved states.
    Diff(TurnSpan),
    /// Count files affected by a proposed rewind.
    Plan(u64),
    /// Restore a confirmed target after preserving the current files.
    Rewind(u64),
    /// Refresh provenance before navigating to an attributed tool.
    Reveal(u64),
}

/// The result to fold into the window after a checkpoint job.
pub(super) struct CheckpointBack {
    /// The operation that produced this result.
    work: CheckpointWork,
    /// Git's answer or diagnostic.
    said: pm_core::Said,
    /// Persisted turn history, read off the UI thread.
    turns: Vec<Checkpoint>,
    /// A comparison, when requested.
    diff: Option<TurnDiff>,
    /// Files affected by a proposed rewind.
    paths: Vec<PathBuf>,
    /// The chain provenance, including an in-progress turn.
    steps: Vec<CheckpointStep>,
    /// Context correction consumed only after a successful new baseline.
    context: Option<String>,
}

impl CheckpointWork {
    /// Runs one checkpoint operation without holding up the window.
    pub(super) fn run(self, root: &pm_host::Location) -> CheckpointBack {
        let roots = pm_core::repositories(root);
        let number = roots
            .iter()
            .map(pm_core::next_checkpoint_number)
            .max()
            .unwrap_or(1);
        let mut back = CheckpointBack {
            work: self.clone(),
            said: Ok(if roots.is_empty() {
                "0".to_owned()
            } else {
                String::new()
            }),
            turns: Vec::new(),
            diff: None,
            paths: Vec::new(),
            steps: Vec::new(),
            context: None,
        };
        for repository in roots {
            let read = self.clone().run_in(&repository, number);
            if back.said.is_ok() {
                back.said = read.said;
            }
            for turn in read.turns {
                if let Some(held) = back.turns.iter_mut().find(|held| held.turn == turn.turn) {
                    held.steps.extend(turn.steps);
                } else {
                    back.turns.push(turn);
                }
            }
            if let Some(diff) = read.diff {
                back.diff
                    .get_or_insert_with(TurnDiff::default)
                    .files
                    .extend(diff.files);
            }
            back.paths.extend(read.paths.into_iter().map(|path| {
                repository
                    .join(&path)
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_path_buf()
            }));
            back.steps.extend(read.steps);
            back.context = back.context.or(read.context);
        }
        if back.said.is_err() {
            back.diff = None;
        }
        back.turns.sort_by_key(|turn| turn.turn);
        back
    }

    /// Performs one operation in one repository, preserving scope-wide turn numbering.
    fn run_in(self, root: &pm_host::Location, number: u64) -> CheckpointBack {
        let mut diff = None;
        let mut paths = Vec::new();
        let said = match &self {
            Self::Begin(_, _) => {
                pm_core::begin_checkpoint_number(root, number).map(|turn| turn.to_string())
            }
            Self::Step(turn, call) => pm_core::checkpoint_step(
                root,
                *turn,
                &call.title,
                (!call.id.is_empty()).then_some(call.id.as_str()),
                Some(&format!("{:?}", call.kind)),
            ),
            Self::End(turn, prompt) => pm_core::end_checkpoint(root, *turn, prompt),
            Self::List(_) | Self::Reveal(_) => Ok(String::new()),
            Self::Diff(span) => {
                let turns = pm_core::checkpoints(root);
                match (
                    turns.iter().find(|turn| turn.turn == span.from),
                    turns.iter().find(|turn| turn.turn == span.to),
                ) {
                    (Some(from), Some(to)) => {
                        let base = if span.baseline {
                            &to.start
                        } else {
                            &from.commit
                        };
                        let steps = pm_core::checkpoint_steps(root);
                        let files = pm_core::between(root, base, &to.commit)
                            .into_iter()
                            .map(|(path, hunks)| {
                                let attribution =
                                    pm_core::hunk_steps(root, &path, &to.commit, &hunks, &steps);
                                let hunks = hunks.into_iter().zip(attribution).collect();
                                (path, hunks)
                            })
                            .collect();
                        diff = Some(TurnDiff {
                            files,
                            ..TurnDiff::default()
                        });
                        Ok(String::new())
                    }
                    _ => Err("The requested turns have not been checkpointed".to_owned()),
                }
            }
            Self::Plan(turn) => pm_core::checkpoint_at(root, *turn)
                .ok_or_else(|| "No checkpoint for this turn".to_owned())
                .and_then(|to| {
                    let now = pm_core::snapshot(root).ok_or("Could not snapshot the worktree")?;
                    paths = pm_core::rewind_paths(root, &now, &to)?;
                    Ok(String::new())
                }),
            Self::Rewind(turn) => pm_core::rewind_number(
                root,
                &format!("refs/worktree/pandemonium/turns/{turn}"),
                number,
            ),
        };
        let context = (matches!(self, Self::Begin(..)) && said.is_ok())
            .then(|| pm_core::take_rewind_context(root))
            .flatten();
        CheckpointBack {
            work: self,
            said,
            turns: pm_core::checkpoints(root),
            diff,
            paths,
            steps: pm_core::checkpoint_steps(root),
            context,
        }
    }
}

impl App {
    /// Enqueues a checkpoint job through the existing per-scope read serialization.
    fn checkpoint_later(&mut self, scope: Scope, work: CheckpointWork) {
        self.readings
            .checkpoints
            .entry(scope)
            .or_default()
            .push_back(work);
        self.work_next(scope);
    }

    /// Captures finished calls and turn boundaries, then prepares pending prompts.
    pub(super) fn hear_checkpoint_moments(&mut self) {
        let abandoned = self
            .checkpointing
            .active
            .iter()
            .filter(|(talk, _)| self.agents.get(**talk).is_none())
            .map(|(talk, turn)| (*talk, turn.clone()))
            .collect::<Vec<_>>();
        for (talk, (scope, turn, prompt)) in abandoned {
            self.checkpointing.active.remove(&talk);
            self.checkpoint_later(scope, CheckpointWork::End(turn, prompt));
        }
        let moments = std::mem::take(&mut self.agents.checkpoint_moments);
        let mut batched = BTreeMap::<TalkId, usize>::new();
        for (talk, call) in &moments {
            if call.is_some() {
                *batched.entry(*talk).or_default() += 1;
            }
        }
        for (talk, mut call) in moments {
            if (batched.get(&talk).is_some_and(|count| *count > 1)
                || self
                    .agents
                    .get(talk)
                    .is_some_and(|talk| talk.has_active_file_tools()))
                && let Some(call) = &mut call
            {
                call.id.clear();
                call.title = "Tool activity between checkpoints".to_owned();
            }
            let Some((scope, turn, prompt)) = self.checkpointing.active.get(&talk).cloned() else {
                continue;
            };
            let work = match call {
                Some(call) => CheckpointWork::Step(turn, Box::new(call)),
                None => {
                    self.checkpointing.active.remove(&talk);
                    CheckpointWork::End(turn, prompt)
                }
            };
            self.checkpoint_later(scope, work);
        }
        let pending = self
            .agents
            .iter()
            .filter_map(|talk| {
                let (text, _) = talk.pending_prompt.as_ref()?;
                let scope = talk.scope();
                if self.checkpointing.starting.contains(&talk.id())
                    || self.checkpointing.rewinding.contains(&scope)
                {
                    return None;
                }
                Some((talk.id(), scope, text.clone()))
            })
            .collect::<Vec<_>>();
        for (talk, scope, prompt) in pending {
            self.checkpointing.starting.insert(talk);
            self.checkpoint_later(scope, CheckpointWork::Begin(talk, prompt));
        }
    }

    /// Applies a completed checkpoint operation and exposes errors to its reader.
    pub(super) fn take_checkpointed(&mut self, scope: Scope, back: CheckpointBack) {
        self.checkpointing.turns.insert(scope, back.turns);
        self.checkpointing.steps.insert(scope, back.steps);
        if let Err(error) = &back.said {
            self.notices.trouble(error.clone(), None);
        }
        match back.work {
            CheckpointWork::Begin(talk, prompt) => {
                self.checkpointing.starting.remove(&talk);
                if let Ok(turn) = back
                    .said
                    .and_then(|turn| turn.parse::<u64>().map_err(|error| error.to_string()))
                {
                    if let Some(talking) = self
                        .agents
                        .get_mut(talk)
                        .filter(|talk| talk.pending_prompt.is_some() && talk.is_busy())
                    {
                        if turn > 0 {
                            self.checkpointing
                                .active
                                .insert(talk, (scope, turn, prompt));
                        }
                        talking.rewind_note = talking.rewind_note.take().or(back.context);
                        talking.checkpoint_ready(turn);
                    }
                } else if let Some(talking) = self.agents.get_mut(talk) {
                    talking.checkpoint_failed();
                }
            }
            CheckpointWork::Diff(span) => {
                if let Some(diff) = back.diff {
                    self.checkpointing.diffs.insert((scope, span), diff);
                    self.open_beside(self.panes.focus(), Item::Turns(scope, span));
                }
            }
            CheckpointWork::List(from) => self.offer_turns(scope, from),
            CheckpointWork::Reveal(prefix) => self.reveal_checkpoint_step(scope, prefix),
            CheckpointWork::Plan(turn) => {
                if back.said.is_ok() && !self.agent_writing(scope) {
                    self.ask_first(Prompt::asking(
                        format!(
                            "Rewind to the end of turn {turn}? {} files change.",
                            back.paths.len()
                        ),
                        back.paths
                            .iter()
                            .map(|path| path.display().to_string())
                            .collect(),
                        vec![
                            Answer::new("Rewind", Message::ConfirmRewind(scope, turn)),
                            Answer::cancel(),
                        ],
                    ));
                }
                self.checkpointing.rewinding.remove(&scope);
            }
            CheckpointWork::Rewind(turn) => {
                self.checkpointing.rewinding.remove(&scope);
                if back.said.is_ok() {
                    let talks = self
                        .agents
                        .iter()
                        .filter(|talk| talk.scope() == scope)
                        .map(|talk| talk.id())
                        .collect::<Vec<_>>();
                    for talk in talks {
                        if let Some(talk) = self.agents.get_mut(talk) {
                            talk.rewound(turn);
                        }
                    }
                    self.reread_changes();
                }
            }
            _ => {}
        }
        self.request_redraw();
    }

    /// Reads a restored turn pane without depending on a loaded conversation.
    pub(super) fn read_turn_diff(&mut self, scope: Scope, span: TurnSpan) {
        self.checkpoint_later(scope, CheckpointWork::Diff(span));
    }

    /// Opens exactly the agent changes of one prompt, excluding earlier reader edits.
    pub(super) fn diff_agent_turn(&mut self, talk: TalkId, turn: u64) {
        if let Some(talk) = self.agents.get(talk) {
            self.checkpoint_later(
                talk.scope(),
                CheckpointWork::Diff(TurnSpan {
                    from: turn.saturating_sub(1),
                    to: turn,
                    baseline: true,
                }),
            );
        }
    }

    /// Whether an agent or its baseline is currently writing this worktree.
    fn agent_writing(&self, scope: Scope) -> bool {
        self.agents
            .iter()
            .any(|talk| talk.scope() == scope && talk.is_busy())
    }

    /// Refuses an active agent, otherwise counts the files a rewind would affect.
    pub(super) fn ask_rewind_agent(&mut self, talk: TalkId, turn: u64) {
        if let Some(talk) = self.agents.get(talk) {
            self.plan_rewind(talk.scope(), turn.saturating_sub(1));
        }
    }

    /// Prepares a recoverable rewind from either a transcript or the history picker.
    pub(super) fn plan_rewind(&mut self, scope: Scope, turn: u64) {
        if self.agent_writing(scope) {
            self.notices.trouble("Stop the agent first", None);
            return;
        }
        if self.checkpointing.rewinding.insert(scope) {
            self.checkpoint_later(scope, CheckpointWork::Plan(turn));
        }
    }

    /// Rechecks agent state before performing the confirmed filesystem rewind.
    pub(super) fn confirm_rewind(&mut self, scope: Scope, turn: u64) {
        if self.agent_writing(scope) {
            self.notices.trouble("Stop the agent first", None);
            return;
        }
        if self.checkpointing.rewinding.insert(scope) {
            self.checkpoint_later(scope, CheckpointWork::Rewind(turn));
        }
    }

    /// Reads durable history before offering the first comparison endpoint.
    pub(super) fn compare_turns(&mut self) {
        if let Some(scope) = self.scope() {
            self.checkpoint_later(scope, CheckpointWork::List(None));
        }
    }

    /// Offers completed turns and saved rewind states as comparison endpoints.
    fn offer_turns(&mut self, scope: Scope, from: Option<u64>) {
        let rows = self
            .checkpointing
            .turns
            .get(&scope)
            .into_iter()
            .flatten()
            .map(|turn| Row {
                section: None,
                label: format!("Turn {} · {}", turn.turn, turn.prompt),
                detail: turn.commit.chars().take(8).collect(),
                choice: Choice::Checkpoint(scope, from, turn.turn),
                enabled: true,
            })
            .collect();
        self.open_picker_with(Kind::Turns, rows, String::new());
    }

    /// Advances the endpoint picker or opens its comparison.
    pub(super) fn choose_turn(&mut self, scope: Scope, from: Option<u64>, turn: u64) {
        match from {
            None => self.offer_turns(scope, Some(turn)),
            Some(from) => self.checkpoint_later(
                scope,
                CheckpointWork::Diff(TurnSpan {
                    from,
                    to: turn,
                    baseline: false,
                }),
            ),
        }
    }

    /// Scrolls an open read-only comparison and reports whether one was targeted.
    pub(super) fn scroll_turns(&mut self, rows: isize) -> bool {
        let Some(Item::Turns(scope, span)) = self.item_under() else {
            return false;
        };
        if let Some(diff) = self.checkpointing.diffs.get_mut(&(scope, span)) {
            let total = diff.row_count(self.preferences.split_diff);
            diff.scroll.by(rows, total);
        }
        true
    }

    /// Opens the tool's owning transcript and reveals its block by provider id.
    pub(super) fn show_checkpoint_step(&mut self, scope: Scope, prefix: u64) {
        self.checkpoint_later(scope, CheckpointWork::Reveal(prefix));
    }

    /// Navigates using provenance already read from git away from the window.
    fn reveal_checkpoint_step(&mut self, scope: Scope, prefix: u64) {
        let tool = self
            .checkpointing
            .steps
            .get(&scope)
            .into_iter()
            .flatten()
            .find(|step| crate::review::step_prefix(step) == prefix)
            .and_then(|step| step.tool.clone());
        let Some(tool) = tool else {
            return;
        };
        let talk = self
            .agents
            .iter()
            .find(|talk| {
                talk.scope() == scope
                    && talk.transcript().blocks().iter().any(
                        |block| matches!(block, crate::agent::Block::Ran(call) if call.id == tool),
                    )
            })
            .map(|talk| talk.id());
        if let Some(talk) = talk {
            self.show_agent(talk);
            if let Some(talk) = self.agents.get_mut(talk) {
                talk.expand_checkpoint_tool(&tool);
            }
            let width = self
                .agents
                .get(talk)
                .map_or(600.0, |talk| talk.view().get().size.width);
            let offset = self
                .agents
                .get(talk)
                .and_then(|talk| crate::agent::tool_offset(&self.theme(), talk, width, &tool));
            if let Some(offset) =
                offset.and_then(|offset| self.agents.get_mut(talk).map(|talk| (talk, offset)))
            {
                offset.0.reveal_tool(&tool, offset.1);
            }
        }
    }
}
