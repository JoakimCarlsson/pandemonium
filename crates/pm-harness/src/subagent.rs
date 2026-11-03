//! Subagents an agent starts, and the tools each of them uses, folded into cards of their own.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::chat::{Event, Kind, Tool, ToolState, tool};

/// The child sessions an agent announced, each against the card that stands for it.
#[derive(Default)]
pub(crate) struct Subagents {
    /// The card of each announced child session.
    cards: BTreeMap<String, String>,
}

impl Subagents {
    /// What an update said, as the events it comes to, with a child's work set under its card.
    ///
    /// `session` is the conversation the reader holds and `update` is read
    /// from a notification for `from`: the child's own prose never joins the
    /// parent's reply.
    pub(crate) fn events(&mut self, session: &str, from: &str, update: &Value) -> Vec<Event> {
        match update["sessionUpdate"].as_str() {
            Some("subagent_spawned") => self.spawned(update).into_iter().collect(),
            Some("subagent_state_update") => self.state(update).into_iter().collect(),
            Some("tool_call" | "tool_call_update") if from != session => {
                let mut event = tool(update);
                if let (Some(held), Some(card)) = (event.as_mut(), self.cards.get(from)) {
                    held.parent = Some(card.clone());
                }
                event.map(Event::Tool).into_iter().collect()
            }
            _ => Vec::new(),
        }
    }

    /// Whether `from` is a child session, whose words are not the reply's.
    pub(crate) fn is_child(&self, from: &str) -> bool {
        self.cards.contains_key(from)
    }

    /// The card a newly announced child gets.
    fn spawned(&mut self, update: &Value) -> Option<Event> {
        let child = update["subagentSessionId"].as_str()?;
        let id = format!("subagent:{child}");
        self.cards.insert(child.to_owned(), id.clone());
        let name = update["name"].as_str().unwrap_or("Subagent").to_owned();
        Some(Event::Tool(Tool {
            id,
            title: Some(name.clone()),
            kind: Some(Kind::Other),
            state: Some(ToolState::Running),
            target: Some(name),
            diff: None,
            output: None,
            parent: None,
            agent: Some(
                update["subagentType"]
                    .as_str()
                    .unwrap_or("agent")
                    .to_owned(),
            ),
        }))
    }

    /// The state a child's card moves to when its lifetime changes.
    fn state(&self, update: &Value) -> Option<Event> {
        let id = self.cards.get(update["subagentSessionId"].as_str()?)?;
        let state = match update["state"].as_str()? {
            "completed" => ToolState::Done,
            "failed" | "cancelled" | "disconnected" => ToolState::Failed,
            "running" => ToolState::Running,
            _ => return None,
        };
        Some(Event::Tool(Tool {
            id: id.clone(),
            title: None,
            kind: None,
            state: Some(state),
            target: None,
            diff: None,
            output: None,
            parent: None,
            agent: None,
        }))
    }
}
