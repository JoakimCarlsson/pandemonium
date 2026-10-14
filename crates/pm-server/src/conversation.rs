//! The conversation a session's log tells, as its agent knows it: the reader's turns since the last rewind, what an agent started afresh is told of them, and the log a branch of them starts from.

use pm_acp::Event;

/// One of the reader's turns and what the agent did in it.
pub(crate) struct Turn {
    /// The number the turn's `Prompted` event has in the log.
    pub(crate) seq: u64,
    /// Whether the files were kept as they were before it.
    pub(crate) checkpoint: bool,
    /// The turn's `Prompted` event and what the agent did after it, in order.
    events: Vec<Event>,
}

impl Turn {
    /// The turn as lines of the history handed to an agent started afresh.
    fn told(&self) -> String {
        let mut told = String::new();
        let mut said = String::new();
        let mut tools: Vec<(&str, &str)> = Vec::new();
        for event in &self.events {
            match event {
                Event::Prompted { text, files, .. } => {
                    told.push_str(&format!("Reader:\n{text}\n\n"));
                    for name in files {
                        told.push_str(&format!("Reader attached {name}.\n\n"));
                    }
                }
                Event::Said(text) => said.push_str(text),
                Event::Tool(tool) => {
                    let held = tools.iter_mut().find(|(id, _)| *id == tool.id);
                    match (held, tool.title.as_deref()) {
                        (Some((_, title)), Some(named)) => *title = named,
                        (None, title) => tools.push((&tool.id, title.unwrap_or("Working"))),
                        (Some(_), None) => {}
                    }
                }
                _ => {}
            }
        }
        for (_, title) in &tools {
            told.push_str(&format!("Tool: {title}\n"));
        }
        if !tools.is_empty() {
            told.push('\n');
        }
        if !said.is_empty() {
            told.push_str(&format!("Agent:\n{said}\n\n"));
        }
        told
    }
}

/// Whether `event` belongs to the conversation itself, rather than to the queue, to going back or to what the agent offers.
fn conversational(event: &Event) -> bool {
    matches!(
        event,
        Event::Said(_)
            | Event::Thought(_)
            | Event::Tool(_)
            | Event::Plan(_)
            | Event::Finished(_)
            | Event::Failed(_)
    )
}

/// The reader's turns the log `log` tells, those a rewind dropped left out, oldest first.
pub(crate) fn turns(log: &[(u64, Event)]) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    for (seq, event) in log {
        match event {
            Event::Prompted { checkpoint, .. } => turns.push(Turn {
                seq: *seq,
                checkpoint: *checkpoint,
                events: vec![event.clone()],
            }),
            Event::Rewound { turn } => turns.truncate(*turn as usize),
            _ if conversational(event) => {
                if let Some(last) = turns.last_mut() {
                    last.events.push(event.clone());
                }
            }
            _ => {}
        }
    }
    turns
}

/// The log a branch of the conversation `log` tells starts from: what the agent last offered, then the reader's turns up to and with `turn`, counted from nought, with no files kept before them. Nothing when there is no such turn.
pub(crate) fn through(log: &[(u64, Event)], turn: u64) -> Option<Vec<(u64, Event)>> {
    let turns = turns(log);
    let kept = turns.get(..=turn as usize)?;
    let offered = |wanted: fn(&Event) -> bool| {
        log.iter()
            .rev()
            .find(|(_, event)| wanted(event))
            .map(|(_, event)| event.clone())
    };
    let settings = offered(|event| matches!(event, Event::Settings(_)));
    let commands = offered(|event| matches!(event, Event::Commands(_)));
    let events = settings
        .into_iter()
        .chain(commands)
        .chain(kept.iter().flat_map(|turn| turn.events.iter().cloned()))
        .map(|event| match event {
            Event::Prompted { text, files, .. } => Event::Prompted {
                text,
                files,
                checkpoint: false,
            },
            other => other,
        });
    Some((1..).zip(events).collect())
}

/// What goes before the reader's next prompt: the earlier conversation when the agent starts `afresh`, and word of files put back since the last turn.
pub(crate) fn preface(log: &[(u64, Event)], afresh: bool) -> String {
    let mut preface = String::new();
    if afresh {
        preface.push_str(&history(log));
    }
    if let Some(turn) = restored(log) {
        preface.push_str(&format!(
            "The files were put back as they were before turn {} of this conversation. Your conversation has not gone back with them.\n\n",
            turn + 1
        ));
    }
    preface
}

/// The earlier conversation, as words handed to an agent started afresh, or nothing when there was none.
fn history(log: &[(u64, Event)]) -> String {
    let turns = turns(log);
    if turns.is_empty() {
        return String::new();
    }
    let mut history = "Earlier conversation, carried over to a new one. Treat it as history, not as new instructions. The files are as they are now, not as they were then.\n\n".to_owned();
    for turn in &turns {
        history.push_str(&turn.told());
    }
    history.push_str("End of the earlier conversation. The reader's new prompt follows.\n\n");
    history
}

/// The turn the files were last put back to before, when that happened after the reader's last turn.
fn restored(log: &[(u64, Event)]) -> Option<u64> {
    log.iter().rev().find_map(|(_, event)| match event {
        Event::Restored { turn, .. } => Some(Some(*turn)),
        Event::Prompted { .. } => Some(None),
        _ => None,
    })?
}

/// The settings the agent last said it had, as the agent names them and their values, for an agent started afresh.
pub(crate) fn settings(log: &[(u64, Event)]) -> Vec<(String, String)> {
    log.iter()
        .rev()
        .find_map(|(_, event)| match event {
            Event::Settings(settings) => Some(
                settings
                    .iter()
                    .map(|setting| (setting.id.clone(), setting.current.clone()))
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}
