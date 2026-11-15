//! One session's conversation and activity, built from the events the server
//! replays and streams for it.

use pm_harness::{Event, Part, SlotInfo, SubagentState};

use crate::composer::{
    QueuedPrompt, SlashCommand, agent_settings, allowance_gauge, context_gauge, goal_chip,
    with_slot_picks,
};

use crate::conversation::{
    Activity, ActivityKind, Block, Extras, Message, Notice, NoticeKind, Speaker, StepState,
    ToolStep,
};
use crate::model::SessionStatus;
use crate::translate;

/// What became of a model that failed, in a few words, as `reason`, the provider's first line, says: overloaded, rate limited, out of reach, or failing.
fn fallback_cause(reason: &str) -> &'static str {
    let reason = reason.to_lowercase();
    if reason.contains("overload") || reason.contains("529") {
        "was overloaded"
    } else if reason.contains("rate limit") || reason.contains("429") {
        "was rate limited"
    } else if ["not found", "unavailable", "404", "503"]
        .iter()
        .any(|word| reason.contains(word))
    {
        "could not be reached"
    } else {
        "kept failing"
    }
}

/// `millis` milliseconds as a reader says it: `0.4s` under a second, `12s` under a minute, `3m 5s` under an hour and `1h 2m` beyond.
fn duration(millis: u64) -> String {
    let seconds = millis / 1000;
    match seconds {
        0 => format!("{:.1}s", millis as f64 / 1000.0),
        1..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3600, seconds / 60 % 60),
    }
}

/// What the window knows of one session from its events.
#[derive(Clone, Debug, Default)]
pub struct Thread {
    /// The messages so far, oldest first.
    pub messages: Vec<Message>,
    /// The cards, strips and bars around the conversation.
    pub extras: Extras,
    /// The text of the block of the agent's reply being written.
    said: String,
    /// What kind of block the agent is writing.
    part: Option<Part>,
    /// Whether the agent said it starts a new block, so the next text or thought does not add to the last.
    fresh: bool,
    /// How many of the reader's turns there have been.
    turns: u64,
    /// The number of the last event taken.
    pub seq: u64,
    /// The title the agent last gave it, if any.
    pub title: Option<String>,
    /// The agent's model, as the reader knows it, once it said.
    pub model: Option<String>,
    /// The agent's mode, such as its permissions, once it said.
    pub mode: Option<String>,
    /// Whether the agent is in a turn.
    working: bool,
    /// Whether the agent waits on an answer: a permission, inquiry, trust or sign-in.
    asking: bool,
    /// Whether the last turn ended in a failure.
    failed: bool,
    /// How many turns have ended.
    updates: u64,
    /// How many ended turns have been read.
    read_through: u64,
    /// Whether the reader marked it unread, whatever has been read.
    unread: bool,
    /// The model slots the agent's harness holds, offered as `@slot` in the model picker.
    slots: Vec<SlotInfo>,
}

impl Thread {
    /// Takes event number `seq`, ignoring ones already taken. Returns whether it was new.
    pub fn take(&mut self, seq: u64, event: Event) -> bool {
        if seq <= self.seq {
            return false;
        }
        self.seq = seq;
        match event {
            Event::Prompted {
                text,
                files,
                checkpoint,
                ..
            } => {
                self.said.clear();
                self.part = None;
                self.working = true;
                self.extras.composer.working = true;
                self.extras.tool_sign_in = None;
                self.failed = false;
                self.messages.push(Message {
                    speaker: Speaker::User,
                    blocks: vec![Block::markdown(text)],
                    footer: None,
                    turn: Some(self.turns),
                    checkpoint,
                    files,
                });
                self.turns += 1;
            }
            Event::Started(part) => {
                self.said.clear();
                self.part = Some(part);
                self.fresh = true;
                self.busy();
                self.agent_message();
            }
            Event::Said(chunk) => {
                self.busy();
                if self.part != Some(Part::Text) {
                    self.part = Some(Part::Text);
                    self.said.clear();
                }
                self.said.push_str(&chunk);
                let block = Block::markdown(self.said.clone());
                self.write_block(block, |block| matches!(block, Block::Markdown { .. }));
            }
            Event::Thought(chunk) => {
                self.busy();
                if self.part != Some(Part::Thinking) {
                    self.part = Some(Part::Thinking);
                    self.said.clear();
                }
                self.said.push_str(&chunk);
                let block = Block::Thought(self.said.clone());
                self.write_block(block, |block| matches!(block, Block::Thought(_)));
            }
            Event::Tool(tool) => {
                self.busy();
                self.part = None;
                self.said.clear();
                if let Some(step) = self.step_mut(&tool.id) {
                    translate::update_step(step, &tool);
                } else if let Some(parent) = tool
                    .parent
                    .clone()
                    .and_then(|parent| self.step_mut(&parent))
                {
                    parent.children.push(translate::tool_step(&tool));
                } else {
                    self.agent_message()
                        .blocks
                        .push(Block::Tool(translate::tool_step(&tool)));
                }
            }
            Event::Background(background) => {
                self.track(
                    background.id,
                    ActivityKind::Background,
                    background.label,
                    background.running,
                );
            }
            Event::Subagent(subagent) => {
                let running = subagent.state == SubagentState::Running;
                if let Some(step) = self.step_mut(&subagent.call) {
                    step.subagent = running.then(|| subagent.id.clone());
                    if step.agent.is_none() {
                        step.agent = Some(subagent.agent.clone());
                    }
                }
                self.track(subagent.id, ActivityKind::Subagent, subagent.label, running);
            }
            Event::Plan(steps) => self.extras.plan = translate::plan(&steps),
            Event::Ask(permission) => {
                self.asking = true;
                self.extras.approval = Some(translate::approval(&permission));
            }
            Event::Inquire(inquiry) => {
                self.asking = true;
                self.extras.inquiry = Some(translate::inquiry(&inquiry));
            }
            Event::Answered(answers) => {
                self.asking = false;
                self.extras.inquiry = None;
                let detail = answers
                    .into_iter()
                    .map(|(question, answer)| {
                        format!("{question}: {}", answer.as_deref().unwrap_or("skipped"))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.notice(
                    NoticeKind::Answered,
                    "Answered the agent's questions",
                    detail,
                );
            }
            Event::Trust(request) => {
                self.asking = true;
                self.extras.trust = Some(translate::trust(&request));
            }
            Event::Trusted { ticket, .. } => {
                if self
                    .extras
                    .trust
                    .as_ref()
                    .is_some_and(|trust| trust.ticket == ticket)
                {
                    self.extras.trust = None;
                    self.asking = false;
                }
            }
            Event::SignedIn => {
                self.extras.signed_out = None;
                self.asking = false;
            }
            Event::SignedOut { reason, account } => {
                self.end_turn(StepState::Failed, "Signed out");
                self.extras.signed_out = Some(match account {
                    Some(account) => format!("{account}: {reason}"),
                    None => reason,
                });
                self.asking = true;
            }
            Event::Failed(reason) => {
                self.failed = true;
                self.notice(NoticeKind::Failure, "The agent failed", reason);
                self.end_turn(StepState::Failed, "Failed");
            }
            Event::Container(step) => self.extras.container = Some(step),
            Event::ToolSignIn(page) => self.extras.tool_sign_in = Some(page),
            Event::Limits(pm_harness::Allowance(limits)) => {
                if let Some(gauge) = allowance_gauge(&limits) {
                    self.extras.composer.allowance = Some(gauge);
                }
            }
            Event::Goal(goal) => self.extras.composer.goal = goal_chip(&goal),
            Event::Slots(slots) => self.set_slots(slots),
            Event::Restored { turn, files } => self.notice(
                NoticeKind::Files,
                format!("Put the files back as they were before turn {}", turn + 1),
                files.join("\n"),
            ),
            Event::Undone { turn, files } => self.notice(
                NoticeKind::Files,
                format!("Took turn {}'s changes back out", turn + 1),
                files.join("\n"),
            ),
            Event::Spent(spent) => self.extras.spend = Some(translate::spend(&spent)),
            Event::ModelFallback {
                slot: _,
                from,
                to,
                reason,
            } => self.notice(
                NoticeKind::Fallback,
                format!("Switched to {to}: {from} {}", fallback_cause(&reason)),
                reason,
            ),
            Event::Aside {
                question,
                answer,
                failed,
            } => {
                let title = if question.is_empty() {
                    "Side question".to_owned()
                } else {
                    question
                };
                if failed {
                    self.notice(NoticeKind::Failure, title, answer);
                } else {
                    self.notice(NoticeKind::Aside, title, String::new());
                    self.agent_message().blocks.push(Block::markdown(answer));
                    self.fresh = true;
                }
            }
            Event::HandedOff { prompt, .. } => {
                self.notice(
                    NoticeKind::HandedOff,
                    "Handed over to a fresh conversation",
                    prompt,
                );
            }
            Event::Rewound { turn } => {
                if let Some(cut) = self
                    .messages
                    .iter()
                    .position(|message| message.turn == Some(turn))
                {
                    self.messages.truncate(cut);
                }
                self.turns = turn;
                self.extras.approval = None;
                self.extras.inquiry = None;
            }
            Event::Titled(title) => self.title = Some(title),
            Event::Commands(commands) => {
                self.extras.composer.commands = commands
                    .into_iter()
                    .map(|command| SlashCommand {
                        name: command.name,
                        description: command.description,
                    })
                    .collect();
            }
            Event::Used(usage) => {
                if let Some(gauge) = context_gauge(&usage) {
                    self.extras.composer.context = Some(gauge);
                }
            }
            Event::Queued { id, text, files } => {
                self.extras.composer.queued.retain(|queued| queued.id != id);
                self.extras
                    .composer
                    .queued
                    .push(QueuedPrompt { id, text, files });
            }
            Event::Unqueued(id) => self.extras.composer.queued.retain(|queued| queued.id != id),
            Event::Settings(settings) => {
                self.extras.composer.settings = agent_settings(&settings);
                with_slot_picks(&mut self.extras.composer.settings, &self.slots);
                let named = |category: &str| {
                    settings
                        .iter()
                        .find(|setting| setting.category == category)
                        .map(|setting| {
                            setting
                                .current_name()
                                .unwrap_or(&setting.current)
                                .to_owned()
                        })
                };
                self.model = named("model").or(self.model.take());
                self.mode = named("mode").or(self.mode.take());
            }
            Event::Finished(failure) => match failure {
                Some(reason) => {
                    self.failed = true;
                    self.notice(NoticeKind::Failure, "The turn failed", reason);
                    self.end_turn(StepState::Failed, "Failed");
                }
                None => self.end_turn(StepState::Done, "Worked"),
            },
            Event::Halted => self.end_turn(StepState::Failed, "Stopped"),
            Event::Took(millis) => self.took(millis),
        }
        true
    }

    /// Puts how long the last turn took, `millis` milliseconds, into the footer of its reply, after the verb.
    fn took(&mut self, millis: u64) {
        let Some(message) = self
            .messages
            .iter_mut()
            .rev()
            .find(|message| message.speaker == Speaker::Agent)
        else {
            return;
        };
        let Some(footer) = &message.footer else {
            return;
        };
        let lasted = format!("for {}", duration(millis));
        message.footer = Some(match footer.split_once(" · ") {
            Some((verb, steps)) => format!("{verb} {lasted} · {steps}"),
            None => format!("{footer} {lasted}"),
        });
    }

    /// Keeps `slots` as the model slots the agent's harness holds and offers them in the model picker.
    pub fn set_slots(&mut self, slots: Vec<SlotInfo>) {
        self.slots = slots;
        with_slot_picks(&mut self.extras.composer.settings, &self.slots);
    }

    /// Notes that the agent is working and no longer waits on the reader.
    fn busy(&mut self) {
        self.working = true;
        self.extras.composer.working = true;
        self.asking = false;
        self.extras.approval = None;
        self.extras.container = None;
    }

    /// The step of the tool call `id` in the reply being written or an earlier one, nested or not.
    fn step_mut(&mut self, id: &str) -> Option<&mut ToolStep> {
        self.messages.iter_mut().rev().find_map(|message| {
            message.blocks.iter_mut().find_map(|block| match block {
                Block::Tool(step) => step.find_mut(id),
                _ => None,
            })
        })
    }

    /// Ends the turn: nothing waits, steps still running are marked `closed`, and the reply gets a footer saying `verb` and how many steps it took.
    fn end_turn(&mut self, closed: StepState, verb: &str) {
        self.working = false;
        self.extras.composer.working = false;
        self.asking = false;
        self.part = None;
        self.fresh = false;
        self.extras.approval = None;
        self.extras.container = None;
        self.extras.activity.clear();
        self.updates += 1;
        self.close_steps(closed);
        if let Some(message) = self.messages.last_mut()
            && message.speaker == Speaker::Agent
            && message.footer.is_none()
        {
            let mut steps = 0;
            for block in &mut message.blocks {
                if let Block::Tool(step) = block {
                    step.close(closed);
                    steps += step.count();
                }
            }
            message.footer = Some(match steps {
                0 => verb.to_owned(),
                1 => format!("{verb} · 1 step"),
                _ => format!("{verb} · {steps} steps"),
            });
        }
    }

    /// Marks `settled` every tool step still marked running, once its turn ended.
    fn close_steps(&mut self, settled: StepState) {
        for message in &mut self.messages {
            for block in &mut message.blocks {
                if let Block::Tool(step) = block
                    && step.state == StepState::Running
                {
                    step.state = settled;
                }
            }
        }
    }

    /// The agent message being written, started when the last message is the reader's.
    fn agent_message(&mut self) -> &mut Message {
        if self
            .messages
            .last()
            .is_none_or(|message| message.speaker != Speaker::Agent || message.footer.is_some())
        {
            self.messages.push(Message {
                speaker: Speaker::Agent,
                ..Message::default()
            });
        }
        self.messages
            .last_mut()
            .expect("an agent message was just ensured")
    }

    /// Puts `block` into the agent message being written, in place of its
    /// last block when that one is of the kind `same` accepts.
    fn write_block(&mut self, block: Block, same: impl Fn(&Block) -> bool) {
        let fresh = std::mem::take(&mut self.fresh);
        let message = self.agent_message();
        match message.blocks.last_mut() {
            Some(_) if fresh => message.blocks.push(block),
            Some(last) if same(last) => *last = block,
            _ => message.blocks.push(block),
        }
    }

    /// Sets a notice of `kind` saying `title` and `detail` into the conversation.
    fn notice(&mut self, kind: NoticeKind, title: impl Into<String>, detail: impl Into<String>) {
        self.part = None;
        self.said.clear();
        let notice = Notice {
            kind,
            title: title.into(),
            detail: detail.into(),
        };
        self.agent_message().blocks.push(Block::Notice(notice));
    }

    /// Shows the activity `id` labelled `label` while `running`, and drops it after.
    fn track(&mut self, id: String, kind: ActivityKind, label: String, running: bool) {
        self.extras.activity.retain(|activity| activity.id != id);
        if running {
            self.extras.activity.push(Activity { id, kind, label });
        }
    }

    /// Marks every ended turn as read.
    pub fn mark_read(&mut self) {
        self.read_through = self.updates;
        self.unread = false;
    }

    /// Marks it unread until it is read again.
    pub fn mark_unread(&mut self) {
        self.unread = true;
    }

    /// Whether a turn ended that has not been read, or the reader marked it unread.
    pub fn is_unread(&self) -> bool {
        self.updates > self.read_through || self.unread
    }

    /// The mark the sidebar shows, given whether the server says the agent runs.
    pub fn status(&self, running: bool) -> SessionStatus {
        if self.asking {
            SessionStatus::NeedsInput
        } else if self.working || running {
            SessionStatus::Working
        } else if self.is_unread() {
            if self.failed {
                SessionStatus::Error
            } else {
                SessionStatus::Unread
            }
        } else {
            SessionStatus::Idle
        }
    }
}

#[cfg(test)]
/// Tests for how a thread takes events.
mod tests {
    use super::*;

    /// A tool report with `id`, under the call `parent` when there is one.
    fn tool(id: &str, parent: Option<&str>) -> pm_harness::Tool {
        pm_harness::Tool {
            id: id.to_owned(),
            title: Some(id.to_owned()),
            kind: None,
            state: None,
            target: None,
            diff: None,
            output: None,
            parent: parent.map(str::to_owned),
            agent: None,
            worktree: None,
        }
    }

    /// Each block the agent starts is its own, so two thoughts in a row stay two.
    #[test]
    fn each_thinking_block_is_one_thought() {
        let mut thread = Thread::default();
        thread.take(1, Event::Started(Part::Thinking));
        thread.take(2, Event::Thought("First".to_owned()));
        thread.take(3, Event::Started(Part::Thinking));
        thread.take(4, Event::Thought("Second".to_owned()));
        assert_eq!(
            thread.messages[0].blocks,
            [
                Block::Thought("First".to_owned()),
                Block::Thought("Second".to_owned())
            ]
        );
    }

    /// A failure ends the turn and closes the steps still running.
    #[test]
    fn a_failure_ends_the_turn() {
        let mut thread = Thread::default();
        thread.take(1, Event::Tool(tool("t", None)));
        thread.take(2, Event::Failed("broke".to_owned()));
        assert_eq!(thread.status(false), SessionStatus::Error);
        let Block::Tool(step) = &thread.messages[0].blocks[0] else {
            panic!("expected a step");
        };
        assert_eq!(step.state, StepState::Failed);
        assert_eq!(
            thread.messages[0].footer.as_deref(),
            Some("Failed · 1 step")
        );
    }

    /// The time a turn took goes into its footer after the verb.
    #[test]
    fn the_footer_says_how_long_the_turn_took() {
        let mut thread = Thread::default();
        thread.take(1, Event::Tool(tool("t", None)));
        thread.take(2, Event::Failed("broke".to_owned()));
        thread.take(3, Event::Took(12_400));
        assert_eq!(
            thread.messages[0].footer.as_deref(),
            Some("Failed for 12s · 1 step")
        );
    }

    /// A subagent's calls nest under the call that started it, which can stop it while it runs.
    #[test]
    fn subagent_steps_nest_under_their_call() {
        let mut thread = Thread::default();
        thread.take(1, Event::Tool(tool("task", None)));
        thread.take(
            2,
            Event::Subagent(pm_harness::Subagent {
                id: "s1".to_owned(),
                call: "task".to_owned(),
                agent: "scout".to_owned(),
                label: "Look".to_owned(),
                model: None,
                background: false,
                state: SubagentState::Running,
            }),
        );
        thread.take(3, Event::Tool(tool("read", Some("task"))));
        let Block::Tool(step) = &thread.messages[0].blocks[0] else {
            panic!("expected a step");
        };
        assert_eq!(step.subagent.as_deref(), Some("s1"));
        assert_eq!(step.children.len(), 1);
        assert_eq!(thread.messages[0].blocks.len(), 1);
    }

    /// A finished turn reads unread until marked read, and replays are ignored.
    #[test]
    fn finished_turns_are_unread_until_read() {
        let mut thread = Thread::default();
        assert!(thread.take(1, Event::Said("hi".to_owned())));
        assert_eq!(thread.status(false), SessionStatus::Working);
        assert!(thread.take(2, Event::Finished(None)));
        assert_eq!(thread.status(false), SessionStatus::Unread);
        assert!(!thread.take(2, Event::Finished(None)));
        thread.mark_read();
        assert_eq!(thread.status(false), SessionStatus::Idle);
    }

    /// A failure ends the turn, so the session no longer reads as working, and settles the steps left running.
    #[test]
    fn failures_end_the_turn() {
        let mut thread = Thread::default();
        thread.take(
            1,
            Event::Tool(pm_harness::Tool {
                id: "t".to_owned(),
                title: Some("Run".to_owned()),
                kind: None,
                state: None,
                target: None,
                diff: None,
                output: None,
                parent: None,
                agent: None,
                worktree: None,
            }),
        );
        assert_eq!(thread.status(false), SessionStatus::Working);
        thread.take(2, Event::Failed("gone".to_owned()));
        assert_eq!(thread.status(false), SessionStatus::Error);
        assert!(matches!(
            &thread.messages[0].blocks[0],
            Block::Tool(step) if step.state == StepState::Failed
        ));
    }

    /// Thoughts, text and tool calls land in one agent message in the order they came.
    #[test]
    fn steps_thoughts_and_text_share_a_message() {
        let mut thread = Thread::default();
        thread.take(1, Event::Thought("Think".to_owned()));
        thread.take(2, Event::Thought("ing".to_owned()));
        thread.take(
            3,
            Event::Tool(pm_harness::Tool {
                id: "t".to_owned(),
                title: Some("Read a.rs".to_owned()),
                kind: None,
                state: None,
                target: None,
                diff: None,
                output: None,
                parent: None,
                agent: None,
                worktree: None,
            }),
        );
        thread.take(4, Event::Said("Done".to_owned()));
        assert_eq!(thread.messages.len(), 1);
        let blocks = &thread.messages[0].blocks;
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0], Block::Thought("Thinking".to_owned()));
        assert!(matches!(&blocks[1], Block::Tool(step) if step.title == "Read a.rs"));
        assert!(matches!(&blocks[2], Block::Markdown { source, .. } if source == "Done"));
    }
}
