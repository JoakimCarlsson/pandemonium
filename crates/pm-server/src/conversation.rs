//! The conversation a session's log tells, as its agent knows it: the reader's turns since the last rewind, what an agent started afresh is told of them, and the log a branch of them starts from.

use pm_harness::Event;

/// One of the reader's turns and what the agent did in it.
pub(crate) struct Turn {
    /// The id the harness saves the turn under, which is also the name the files kept before it go by: what the `Prompted` event says, or its number in the log for a turn from before turns had one.
    pub(crate) id: u64,
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
        Event::Started(_)
            | Event::Said(_)
            | Event::Halted
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
            Event::Prompted {
                checkpoint, turn, ..
            } => turns.push(Turn {
                id: if *turn == 0 { *seq } else { *turn },
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

/// The agent's final message of the last turn `log` tells: what it said after the last tool it used, its blocks of text set apart by an empty line.
///
/// # Errors
///
/// Says why there is none: the turn failed, for the reason it gave, it was stopped, it never ended, or the agent said nothing.
pub(crate) fn final_reply(log: &[(u64, Event)]) -> Result<String, String> {
    let turns = turns(log);
    let Some(turn) = turns.last() else {
        return Err("the conversation has no turn".to_owned());
    };
    let mut said = String::new();
    let mut tools: Vec<&str> = Vec::new();
    let mut ended: Option<Option<String>> = None;
    let mut stopped = false;
    let mut failed: Option<String> = None;
    for event in &turn.events {
        match event {
            Event::Said(text) => said.push_str(text),
            Event::Started(pm_harness::Part::Text) if !said.trim().is_empty() => {
                said.push_str("\n\n");
            }
            Event::Tool(tool) if !tools.contains(&tool.id.as_str()) => {
                tools.push(&tool.id);
                said.clear();
            }
            Event::Halted => stopped = true,
            Event::Failed(reason) => failed = Some(reason.clone()),
            Event::Finished(reason) => ended = Some(reason.clone()),
            _ => {}
        }
    }
    match (failed, ended) {
        (Some(reason), _) | (None, Some(Some(reason))) => Err(reason),
        (None, _) if stopped => Err("the summary was stopped".to_owned()),
        (None, None) => Err("the summary did not finish".to_owned()),
        (None, Some(None)) if said.trim().is_empty() => {
            Err("the agent wrote no summary".to_owned())
        }
        (None, Some(None)) => Ok(said.trim().to_owned()),
    }
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
            Event::Prompted {
                text, files, turn, ..
            } => Event::Prompted {
                text,
                files,
                checkpoint: false,
                turn,
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

/// The log of a session with its conversation taken from what the harness saved: the turns the harness has, each as the events a live turn would have told, in place of the ones the log holds; what the harness does not know of, such as queued prompts, stays where it was. The choices the harness saved for the settings are put into the last list of settings the log holds.
///
/// A turn keeps the id the harness saved it under, and whether its files were kept is carried over from the log. A turn the harness dropped, because it was stopped or failed, is gone from the result. The events are numbered again from one.
pub(crate) fn from_saved(
    log: &[(u64, Event)],
    saved: &pm_harness::Conversation,
) -> Vec<(u64, Event)> {
    let kept: std::collections::HashMap<u64, bool> = log
        .iter()
        .filter_map(|(seq, event)| match event {
            Event::Prompted {
                checkpoint, turn, ..
            } => Some((if *turn == 0 { *seq } else { *turn }, *checkpoint)),
            _ => None,
        })
        .collect();
    let first = log
        .iter()
        .position(|(_, event)| matches!(event, Event::Prompted { .. }))
        .unwrap_or(log.len());
    let offered = !saved.offered.is_empty();
    let commanded = offered || !saved.commands.is_empty();
    let measured = saved.window.is_some();
    let other = |event: &Event| {
        !conversational(event)
            && !matches!(event, Event::Prompted { .. } | Event::Rewound { .. })
            && !(offered && matches!(event, Event::Settings(_)))
            && !(commanded && matches!(event, Event::Commands(_)))
            && !(measured && matches!(event, Event::Used(_)))
    };
    let mut events: Vec<Event> = log[..first]
        .iter()
        .map(|(_, event)| event)
        .filter(|event| other(event))
        .cloned()
        .collect();
    if offered {
        events.push(Event::Settings(saved.offered.clone()));
    }
    if commanded {
        events.push(Event::Commands(saved.commands.clone()));
    }
    for turn in &saved.turns {
        let id = turn.id.as_deref().and_then(|id| id.parse::<u64>().ok());
        events.push(Event::Prompted {
            text: turn.prompt.clone(),
            files: turn.files.clone(),
            checkpoint: id.and_then(|id| kept.get(&id).copied()).unwrap_or(false),
            turn: id.unwrap_or(0),
        });
        events.extend(turn.events.iter().cloned().filter_map(Event::from_harness));
        if !matches!(events.last(), Some(Event::Finished(_))) {
            events.push(Event::Finished(None));
        }
    }
    events.extend(
        log[first..]
            .iter()
            .map(|(_, event)| event)
            .filter(|event| other(event))
            .cloned(),
    );
    choose_settings(&mut events, &saved.settings);
    (1..).zip(events).collect()
}

/// The conversation `log` tells, with the name `title`, as the harness is handed it to save as its own, for a session from before the harness kept conversations; nothing when the log has no turn.
///
/// What the agent said in a turn goes in as one reply, since the tools it used were never in the log. A turn that was stopped, failed or left open goes in as lost, with the reason.
pub(crate) fn to_import(log: &[(u64, Event)], title: &str) -> Option<pm_harness::Imported> {
    let held = turns(log);
    if held.is_empty() {
        return None;
    }
    let last = |kind: fn(&Event) -> bool| {
        log.iter()
            .rev()
            .map(|(_, event)| event)
            .find(|event| kind(event))
    };
    let (offered, settings) = match last(|event| matches!(event, Event::Settings(_))) {
        Some(Event::Settings(offered)) => (
            offered.clone(),
            offered
                .iter()
                .map(|setting| (setting.id.clone(), setting.current.clone()))
                .collect(),
        ),
        _ => (Vec::new(), Vec::new()),
    };
    let commands = match last(|event| matches!(event, Event::Commands(_))) {
        Some(Event::Commands(commands)) => commands.clone(),
        _ => Vec::new(),
    };
    let window = match last(|event| matches!(event, Event::Used(_))) {
        Some(Event::Used(usage)) => Some(usage.size),
        _ => None,
    };
    let turns = held
        .iter()
        .map(|turn| {
            let (mut prompt, mut files, mut said) = (String::new(), Vec::new(), String::new());
            let mut stopped = false;
            let mut ended: Option<Option<String>> = None;
            for event in &turn.events {
                match event {
                    Event::Prompted {
                        text, files: names, ..
                    } => {
                        prompt = text.clone();
                        files = names.clone();
                    }
                    Event::Said(text) => said.push_str(text),
                    Event::Halted => stopped = true,
                    Event::Finished(reason) => ended = Some(reason.clone()),
                    _ => {}
                }
            }
            pm_harness::ImportedTurn {
                id: turn.id.to_string(),
                prompt,
                files,
                said,
                end: match (stopped, ended) {
                    (true, _) => Some("stopped".to_owned()),
                    (false, Some(None)) => None,
                    (false, Some(Some(reason))) => Some(reason),
                    (false, None) => Some("interrupted".to_owned()),
                },
            }
        })
        .collect();
    Some(pm_harness::Imported {
        title: Some(title.to_owned()).filter(|title| !title.is_empty()),
        settings,
        offered,
        commands,
        window,
        turns,
    })
}

/// Puts `chosen`, the value the harness saved for each setting, as the current value of that setting in the last list of settings in `events`.
fn choose_settings(events: &mut [Event], chosen: &[(String, String)]) {
    let Some(Event::Settings(settings)) = events
        .iter_mut()
        .rev()
        .find(|event| matches!(event, Event::Settings(_)))
    else {
        return;
    };
    for setting in settings {
        if let Some((_, value)) = chosen.iter().find(|(id, _)| *id == setting.id) {
            setting.current = value.clone();
        }
    }
}

/// Whether `event` is kept in the log on disk: what the harness saves with the conversation, such as its turns, its name, what can be set, the commands it offers and how full its context is, is not.
pub(crate) fn kept_on_disk(event: &Event) -> bool {
    !conversational(event)
        && !matches!(
            event,
            Event::Titled(_) | Event::Settings(_) | Event::Commands(_) | Event::Used(_)
        )
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

#[cfg(test)]
/// Tests for taking a conversation from what the harness saved.
mod tests {
    use super::*;
    use pm_harness::PastTurn;

    /// A turn the reader prompted, with the id it was saved under.
    fn prompted(text: &str, turn: u64, checkpoint: bool) -> Event {
        Event::Prompted {
            text: text.to_owned(),
            files: Vec::new(),
            checkpoint,
            turn,
        }
    }

    /// The conversation comes from the harness, a turn it dropped is gone, kept files and queued prompts carry over, and the events are numbered again.
    #[test]
    fn the_harness_is_the_source_of_the_conversation() {
        let log: Vec<(u64, Event)> = vec![
            (1, Event::Titled("t".to_owned())),
            (2, prompted("one", 2, true)),
            (3, Event::Said("a".to_owned())),
            (4, Event::Finished(None)),
            (5, prompted("two", 5, true)),
            (6, Event::Halted),
            (7, Event::Finished(None)),
            (
                8,
                Event::Queued {
                    id: 1,
                    text: "later".to_owned(),
                    files: Vec::new(),
                },
            ),
            (9, Event::Rewound { turn: 9 }),
        ];
        let past = vec![
            PastTurn {
                id: Some("2".to_owned()),
                prompt: "one".to_owned(),
                files: Vec::new(),
                events: vec![pm_harness::WireEvent::Said("answer".to_owned())],
            },
            PastTurn {
                id: Some("5".to_owned()),
                prompt: "two".to_owned(),
                files: Vec::new(),
                events: vec![
                    pm_harness::WireEvent::Halted,
                    pm_harness::WireEvent::Finished(None),
                ],
            },
        ];
        let saved = pm_harness::Conversation {
            title: None,
            turns: past,
            ..Default::default()
        };
        let rebuilt = from_saved(&log, &saved);
        let events: Vec<&Event> = rebuilt.iter().map(|(_, event)| event).collect();
        assert_eq!(events[0], &Event::Titled("t".to_owned()));
        assert_eq!(events[1], &prompted("one", 2, true));
        assert_eq!(events[2], &Event::Said("answer".to_owned()));
        assert_eq!(events[3], &Event::Finished(None));
        assert_eq!(events[4], &prompted("two", 5, true));
        assert_eq!(events[5], &Event::Halted);
        assert_eq!(events[6], &Event::Finished(None));
        assert!(matches!(events[7], Event::Queued { .. }));
        assert_eq!(events.len(), 8);
        assert_eq!(
            rebuilt.iter().map(|(seq, _)| *seq).collect::<Vec<_>>(),
            (1..=8).collect::<Vec<u64>>()
        );
        assert_eq!(turns(&rebuilt).len(), 2);
    }

    /// The choices the harness saved become the current values of the settings the log last offered, and nothing is saved for the rest.
    #[test]
    fn saved_choices_become_the_current_settings() {
        let offered = |current: &str| {
            Event::Settings(vec![
                pm_harness::Setting {
                    id: "model".to_owned(),
                    category: "model".to_owned(),
                    name: "Model".to_owned(),
                    current: current.to_owned(),
                    picks: Vec::new(),
                },
                pm_harness::Setting {
                    id: "mode".to_owned(),
                    category: "mode".to_owned(),
                    name: "Mode".to_owned(),
                    current: "default".to_owned(),
                    picks: Vec::new(),
                },
            ])
        };
        let log = vec![(1, offered("old"))];
        let saved = pm_harness::Conversation {
            settings: vec![("model".to_owned(), "new".to_owned())],
            ..Default::default()
        };
        let rebuilt = from_saved(&log, &saved);
        let Event::Settings(settings) = &rebuilt[0].1 else {
            panic!("no settings");
        };
        assert_eq!(settings[0].current, "new");
        assert_eq!(settings[1].current, "default");
    }

    /// What the harness saves with the conversation is not written to the log on disk, and what is the server's own is.
    #[test]
    fn only_the_servers_own_events_are_kept_on_disk() {
        assert!(!kept_on_disk(&Event::Said("x".to_owned())));
        assert!(!kept_on_disk(&Event::Finished(None)));
        assert!(!kept_on_disk(&Event::Titled("x".to_owned())));
        assert!(!kept_on_disk(&Event::Settings(Vec::new())));
        assert!(!kept_on_disk(&Event::Commands(Vec::new())));
        assert!(!kept_on_disk(&Event::Used(pm_harness::Usage {
            used: 1,
            size: 2,
            written: 0,
        })));
        assert!(kept_on_disk(&prompted("x", 1, true)));
        assert!(kept_on_disk(&Event::Rewound { turn: 0 }));
        assert!(kept_on_disk(&Event::Queued {
            id: 1,
            text: String::new(),
            files: Vec::new()
        }));
    }

    /// What the harness saved of what can be set, the commands and the context window replaces what the log holds of them, and a log with nothing saved keeps its own.
    #[test]
    fn the_saved_offer_replaces_the_logs() {
        let setting = |id: &str, current: &str| pm_harness::Setting {
            id: id.to_owned(),
            category: id.to_owned(),
            name: id.to_owned(),
            current: current.to_owned(),
            picks: Vec::new(),
        };
        let slash = |name: &str| pm_harness::Slash {
            name: name.to_owned(),
            description: String::new(),
        };
        let log: Vec<(u64, Event)> = vec![
            (1, Event::Settings(vec![setting("model", "old")])),
            (2, Event::Commands(vec![slash("old")])),
            (
                3,
                Event::Used(pm_harness::Usage {
                    used: 1,
                    size: 2,
                    written: 0,
                }),
            ),
        ];
        let saved = pm_harness::Conversation {
            offered: vec![setting("model", "new")],
            commands: vec![slash("fresh")],
            window: Some(1000),
            ..Default::default()
        };
        let rebuilt = from_saved(&log, &saved);
        let events: Vec<&Event> = rebuilt.iter().map(|(_, event)| event).collect();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], Event::Settings(s) if s[0].current == "new"));
        assert!(matches!(events[1], Event::Commands(c) if c[0].name == "fresh"));
        let kept = from_saved(&log, &pm_harness::Conversation::default());
        assert_eq!(kept.len(), 3);
    }

    /// An old log becomes an import: answered turns with what was said, a stopped turn and a failed one with their ends, a turn left open as interrupted, a rewound turn left out, and the last offer, commands and window with it.
    #[test]
    fn an_old_log_becomes_an_import() {
        let said = |text: &str| Event::Said(text.to_owned());
        let log: Vec<(u64, Event)> = vec![
            (
                1,
                Event::Commands(vec![pm_harness::Slash {
                    name: "c".to_owned(),
                    description: String::new(),
                }]),
            ),
            (2, prompted("one", 0, true)),
            (3, said("a")),
            (4, said("b")),
            (5, Event::Finished(None)),
            (6, prompted("two", 0, false)),
            (7, Event::Halted),
            (8, Event::Finished(None)),
            (9, prompted("three", 0, false)),
            (10, Event::Finished(Some("boom".to_owned()))),
            (11, prompted("four", 0, false)),
            (12, said("x")),
            (
                13,
                Event::Used(pm_harness::Usage {
                    used: 1,
                    size: 500,
                    written: 0,
                }),
            ),
        ];
        let imported = to_import(&log, "Old").unwrap();
        let ends: Vec<(&str, &str, Option<&str>)> = imported
            .turns
            .iter()
            .map(|turn| (turn.id.as_str(), turn.said.as_str(), turn.end.as_deref()))
            .collect();
        assert_eq!(
            ends,
            vec![
                ("2", "ab", None),
                ("6", "", Some("stopped")),
                ("9", "", Some("boom")),
                ("11", "x", Some("interrupted")),
            ]
        );
        assert_eq!(imported.title.as_deref(), Some("Old"));
        assert_eq!(imported.commands.len(), 1);
        assert_eq!(imported.window, Some(500));
        let mut rewound = log;
        rewound.push((14, Event::Rewound { turn: 1 }));
        assert_eq!(to_import(&rewound, "").unwrap().turns.len(), 1);
        assert!(to_import(&[], "x").is_none());
    }
}
