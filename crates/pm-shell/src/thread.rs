//! One session's conversation and activity, built from the events the server
//! replays and streams for it.

use pm_harness::{Event, Part, SlotInfo, SubagentState};

use crate::composer::{QueuedPrompt, SlashCommand, agent_settings, context_gauge, with_slot_picks};

use crate::conversation::{
    Activity, ActivityKind, Block, Extras, Message, Notice, NoticeKind, Speaker, StepState,
};
use crate::model::SessionStatus;
use crate::translate;

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
            Event::Prompted { text, .. } => {
                self.said.clear();
                self.part = None;
                self.working = true;
                self.extras.composer.working = true;
                self.failed = false;
                self.messages.push(Message {
                    speaker: Speaker::User,
                    blocks: vec![Block::markdown(text)],
                    footer: None,
                    turn: Some(self.turns),
                });
                self.turns += 1;
            }
            Event::Started(part) => {
                self.said.clear();
                self.part = Some(part);
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
                let known = self.messages.iter_mut().rev().find_map(|message| {
                    message.blocks.iter_mut().find_map(|block| match block {
                        Block::Tool(step) if step.id == tool.id => Some(step),
                        _ => None,
                    })
                });
                match known {
                    Some(step) => translate::update_step(step, &tool),
                    None => self
                        .agent_message()
                        .blocks
                        .push(Block::Tool(translate::tool_step(&tool))),
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
            Event::Answered(_) => {
                self.asking = false;
                self.extras.inquiry = None;
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
            Event::SignedIn => self.busy(),
            Event::SignedOut { .. } => self.asking = true,
            Event::Failed(reason) => {
                self.failed = true;
                self.notice(NoticeKind::Failure, "The agent failed", reason);
                self.end_turn();
            }
            Event::Spent(spent) => self.extras.spend = Some(translate::spend(&spent)),
            Event::ModelFallback {
                slot,
                from,
                to,
                reason,
            } => self.notice(
                NoticeKind::Fallback,
                format!("Switched {slot} from {from} to its backup {to}"),
                reason,
            ),
            Event::Aside {
                question,
                answer,
                failed,
            } => self.notice(
                if failed {
                    NoticeKind::Failure
                } else {
                    NoticeKind::Aside
                },
                if question.is_empty() {
                    "Side question".to_owned()
                } else {
                    question
                },
                answer,
            ),
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
            Event::Finished(failure) => {
                if let Some(reason) = failure {
                    self.failed = true;
                    self.notice(NoticeKind::Failure, "The turn failed", reason);
                }
                self.end_turn();
            }
            Event::Halted => {
                self.close_steps(StepState::Failed);
                self.end_turn();
            }
            _ => {}
        }
        true
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
    }

    /// Ends the turn: nothing waits, nothing runs, and the reply gets its footer.
    fn end_turn(&mut self) {
        self.working = false;
        self.extras.composer.working = false;
        self.asking = false;
        self.part = None;
        self.extras.approval = None;
        self.extras.activity.clear();
        self.updates += 1;
        self.close_steps(if self.failed {
            StepState::Failed
        } else {
            StepState::Done
        });
        if let Some(message) = self.messages.last_mut()
            && message.speaker == Speaker::Agent
        {
            message.footer = Some("Worked".to_owned());
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
        let message = self.agent_message();
        match message.blocks.last_mut() {
            Some(last) if same(last) => *last = block,
            _ => message.blocks.push(block),
        }
    }

    /// Sets a notice of `kind` saying `title` and `detail` into the conversation.
    fn notice(&mut self, kind: NoticeKind, title: impl Into<String>, detail: impl Into<String>) {
        self.part = None;
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
    }

    /// The mark the sidebar shows, given whether the server says the agent runs.
    pub fn status(&self, running: bool) -> SessionStatus {
        if self.asking {
            SessionStatus::NeedsInput
        } else if self.working || running {
            SessionStatus::Working
        } else if self.updates > self.read_through {
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
mod tests {
    use super::*;

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
