//! What a session's conversation becomes as its agent speaks.

use std::time::Instant;

use pm_acp::{Event, Kind as ToolKind, Tool, ToolState};

use crate::sample::{Message, Queued, Session, Status, Step, Thought, ToolUse};

impl Session {
    /// Takes in what the agent did.
    pub fn hear(&mut self, event: Event) {
        if !matches!(event, Event::Thought(_)) {
            self.stop_thinking();
        }
        match event {
            Event::Settings(settings) => self.settings = settings,
            Event::Commands(commands) => self.commands = commands,
            Event::Said(text) => self.say(&text),
            Event::Thought(text) => self.think(text),
            Event::Tool(tool) => self.use_tool(tool),
            Event::Plan(plan) => self.plan(plan),
            Event::Ask(question) => self.ask = Some(question),
            Event::Finished(error) => self.finish(error),
            Event::Failed(reason) => self.finish(Some(reason)),
            Event::Titled(title) => self.title = title,
            Event::Prompted {
                text,
                files,
                checkpoint,
            } => self.prompted(text, files, checkpoint),
            Event::Queued { id, text, files } => self.queued.push(Queued { id, text, files }),
            Event::Unqueued(id) => self.queued.retain(|waiting| waiting.id != id),
            Event::Rewound { turn } => self.rewound(turn),
            Event::Restored { turn, files } => self.files_restored(turn, files.len()),
        }
    }

    /// Drops the reader's turn `turn`, counted from nought, and everything after it.
    fn rewound(&mut self, turn: u64) {
        let at = self
            .messages
            .iter()
            .enumerate()
            .filter(|(_, message)| message.from_user)
            .nth(turn as usize)
            .map(|(index, _)| index);
        if let Some(at) = at {
            self.messages.truncate(at);
        }
        self.ask = None;
        self.echoes = 0;
        self.turn_started = None;
        self.status = Status::Done;
    }

    /// Says that `count` files were put back as they were before the reader's turn `turn`, counted from nought.
    fn files_restored(&mut self, turn: u64, count: usize) {
        let files = if count == 1 { "file" } else { "files" };
        let mut note = Message::reply(&format!(
            "Restored {count} {files} to how they were before turn {}.",
            turn + 1
        ));
        note.duration = Some(String::new());
        self.messages.push(note);
    }

    /// Adds the turn a reader handed the agent, unless this window handed it and already shows it, and notes whether its files were kept.
    fn prompted(&mut self, text: String, files: Vec<String>, checkpoint: bool) {
        if self.echoes > 0 {
            self.echoes -= 1;
            if let Some(shown) = self.messages.iter_mut().rev().find(|held| held.from_user) {
                shown.checkpoint = checkpoint;
            }
            return;
        }
        let mut message = Message::user(&text);
        message.attachments = files;
        message.checkpoint = checkpoint;
        self.messages.push(message);
        self.status = Status::Working;
        self.turn_started = Some(Instant::now());
    }

    /// Notes how long the agent's latest thought went on, once it moves on to something else.
    fn stop_thinking(&mut self) {
        let thought = self
            .messages
            .last_mut()
            .and_then(|reply| reply.steps.last_mut())
            .and_then(|step| match step {
                Step::Thought(thought) if thought.seconds.is_none() => Some(thought),
                _ => None,
            });
        if let Some(thought) = thought {
            thought.seconds = Some(thought.started.elapsed().as_secs());
        }
    }

    /// The reply the agent is writing: the last message while it is the agent's and its turn goes on.
    fn reply(&mut self) -> &mut Message {
        let writing = self
            .messages
            .last()
            .is_some_and(|last| !last.from_user && last.duration.is_none());
        if !writing {
            self.messages.push(Message::reply(""));
        }
        self.messages.last_mut().expect("a reply was just pushed")
    }

    /// Adds `text` to the reply being written, in a paragraph of its own when a step came between.
    fn say(&mut self, text: &str) {
        let reply = self.reply();
        if !reply.text.is_empty() && reply.steps.len() > reply.said_after {
            reply.text.push_str("\n\n");
        }
        reply.text.push_str(text);
        let before = reply.steps.len();
        match reply.said.last_mut() {
            Some((at, piece)) if *at == before => piece.push_str(text),
            _ => reply.said.push((before, text.to_owned())),
        }
        reply.said_after = before;
    }

    /// Adds `text` to what the agent is thinking, or starts a thought.
    fn think(&mut self, text: String) {
        let reply = self.reply();
        match reply.steps.last_mut() {
            Some(Step::Thought(held)) if held.seconds.is_none() => held.text.push_str(&text),
            _ => reply.steps.push(Step::Thought(Thought {
                text,
                started: Instant::now(),
                seconds: None,
            })),
        }
    }

    /// Records what the agent said about a tool: a new step, or more about one it has.
    fn use_tool(&mut self, tool: Tool) {
        let reply = self.reply();
        let held = reply
            .steps
            .iter_mut()
            .filter_map(|step| match step {
                Step::Tool(held) => Some(held),
                _ => None,
            })
            .find(|held| held.id == tool.id);
        match held {
            Some(held) => {
                if let Some(title) = tool.title {
                    held.title = title;
                }
                if let Some(kind) = tool.kind {
                    held.kind = kind;
                }
                if let Some(state) = tool.state {
                    if held.state == ToolState::Running && state != ToolState::Running {
                        held.seconds = Some(held.started.elapsed().as_secs());
                    }
                    held.state = state;
                }
                held.target = tool.target.or(held.target.take());
                held.diff = tool.diff.or(held.diff.take());
                held.output = tool.output.or(held.output.take());
            }
            None => reply.steps.push(Step::Tool(ToolUse {
                id: tool.id,
                title: tool.title.unwrap_or_else(|| "Working".to_owned()),
                kind: tool.kind.unwrap_or(ToolKind::Other),
                state: tool.state.unwrap_or(ToolState::Running),
                target: tool.target,
                diff: tool.diff,
                output: tool.output,
                started: Instant::now(),
                seconds: None,
            })),
        }
    }

    /// Replaces the plan the reply keeps, or starts one.
    fn plan(&mut self, plan: Vec<pm_acp::PlanStep>) {
        let reply = self.reply();
        match reply
            .steps
            .iter_mut()
            .find(|step| matches!(step, Step::Plan(_)))
        {
            Some(held) => *held = Step::Plan(plan),
            None => reply.steps.push(Step::Plan(plan)),
        }
    }

    /// Ends the turn that was running, saying what went wrong where something did.
    fn finish(&mut self, error: Option<String>) {
        let ended = if error.is_some() {
            ToolState::Failed
        } else {
            ToolState::Done
        };
        let took = self
            .turn_started
            .take()
            .map(|started| worked(started.elapsed().as_secs()));
        if let Some(reason) = &error {
            let reply = self.reply();
            if !reply.text.is_empty() {
                reply.text.push_str("\n\n");
            }
            reply
                .text
                .push_str(&format!("Something went wrong: {reason}"));
        }
        if let Some(reply) = self
            .messages
            .last_mut()
            .filter(|last| !last.from_user && last.duration.is_none())
        {
            for step in &mut reply.steps {
                if let Step::Tool(tool) = step
                    && tool.state == ToolState::Running
                {
                    tool.state = ended;
                    tool.seconds = Some(tool.started.elapsed().as_secs());
                }
            }
            reply.duration = Some(took.unwrap_or_default());
        }
        self.ask = None;
        self.status = if self.changes.is_empty() {
            Status::Done
        } else {
            Status::Review
        };
    }
}

/// How long a turn took, in words: `Worked for 4m 12s`.
fn worked(seconds: u64) -> String {
    if seconds >= 60 {
        format!("Worked for {}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("Worked for {seconds}s")
    }
}
