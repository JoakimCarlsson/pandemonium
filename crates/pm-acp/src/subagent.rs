//! Subagent session lifetimes and their projection into nested tool cards.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::update::{self, Event, Output, Status, Tools, Voice};

/// Announced child sessions, mapped to the cards containing their work.
#[derive(Default)]
pub(crate) struct Subagents {
    /// The card identity belonging to each announced child session.
    cards: BTreeMap<String, String>,
}

impl Subagents {
    /// Converts lifecycle and child updates without mixing child prose into the parent reply.
    pub(crate) fn events(&mut self, params: &Value, tools: &mut Tools) -> Vec<Event> {
        let Some(session) = params["sessionId"].as_str() else {
            return Vec::new();
        };
        let update = &params["update"];
        match update["sessionUpdate"].as_str() {
            Some("subagent_state_update") => self.state(update, tools),
            Some(_) => self.event(session, update, tools).into_iter().collect(),
            None => Vec::new(),
        }
    }

    /// Settles an announced child's card at the state its lifetime ended in.
    ///
    /// A child that was cancelled or cut off takes the calls still running
    /// inside it along, since none of them can go on; one that failed leaves
    /// them as they were last reported.
    fn state(&self, update: &Value, tools: &mut Tools) -> Vec<Event> {
        let Some(id) = update["subagentSessionId"]
            .as_str()
            .and_then(|child| self.cards.get(child))
        else {
            return Vec::new();
        };
        let status = match update["state"].as_str() {
            Some("completed") => "completed",
            Some("failed") => "failed",
            Some("running") => "in_progress",
            Some("cancelled") => return update::halt(tools, Some(id), Status::Cancelled),
            Some("disconnected") => return update::halt(tools, Some(id), Status::Disconnected),
            _ => return Vec::new(),
        };
        update::event(
            &json!({
                "sessionUpdate": "tool_call_update",
                "toolCallId": id,
                "status": status,
            }),
            tools,
        )
        .into_iter()
        .collect()
    }

    /// Converts one update other than a child's change of state.
    fn event(&mut self, session: &str, update: &Value, tools: &mut Tools) -> Option<Event> {
        match update["sessionUpdate"].as_str()? {
            "subagent_spawned" => {
                let child = update["subagentSessionId"].as_str()?;
                let id = format!("subagent:{child}");
                let mut card = json!({
                    "sessionUpdate": "tool_call",
                    "toolCallId": id,
                    "name": "Task",
                    "title": update["name"].as_str().unwrap_or("Subagent"),
                    "kind": "other",
                    "status": "in_progress",
                    "rawInput": { "description": update["name"] },
                });
                self.parent(session, &mut card);
                self.cards.insert(child.to_owned(), id);
                update::event(&card, tools)
            }
            "tool_call" | "tool_call_update" => {
                let mut update = update.clone();
                self.parent(session, &mut update);
                update::event(&update, tools)
            }
            _ if self.cards.contains_key(session) => {
                let Event::Said(Voice::Agent, text) = update::event(update, tools)? else {
                    return None;
                };
                let call = tools.get_mut(self.cards.get(session)?)?;
                match call.output.last_mut() {
                    Some(Output::Said(passage)) => passage.push_str(&text),
                    _ => call.output.push(Output::Said(text)),
                }
                Some(Event::Ran(call.clone()))
            }
            _ => update::event(update, tools),
        }
    }

    /// Attaches child tools and permission previews to their announced session card.
    pub(crate) fn parent(&self, session: &str, update: &mut Value) {
        if let Some(id) = self.cards.get(session) {
            update["_meta"]["claudeCode"]["parentToolUseId"] = json!(id);
        }
    }
}
