//! Commands and tasks an agent leaves running beyond the call that started them.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::chat::{Background, Event};

/// The commands running beyond their calls, each by the handle the agent uses for it, against what started it.
pub(crate) type Shells = BTreeMap<String, String>;

/// Reads what one update says of the commands and tasks running beyond the turn.
pub(crate) fn background(update: &Value, shells: &mut Shells) -> Vec<Event> {
    if let Some(event) = async_task(update, shells).or_else(|| notified(update, shells)) {
        return vec![event];
    }
    let ended = reported_over(update, shells);
    if !ended.is_empty() {
        return ended;
    }
    if let Some(event) = reported_running(update, shells) {
        return vec![event];
    }
    if let Some(event) = settled(update, shells) {
        return vec![event];
    }
    background_shell(update, shells).into_iter().collect()
}

/// Takes the commands `running` holds, which a turn that ended left running, as background work.
///
/// Any agent can leave a command running past the end of its turn without
/// saying more of it than the status its call already carries, so a command
/// call still running at that point is the work to follow.
pub(crate) fn outliving(running: &Shells, shells: &mut Shells) -> Vec<Event> {
    let mut events = Vec::new();
    for (id, label) in running {
        if shells.contains_key(id) {
            continue;
        }
        shells.insert(id.clone(), label.clone());
        events.push(Event::Background(Background {
            id: id.clone(),
            label: label.clone(),
            running: true,
            call: Some(id.clone()),
        }));
    }
    events
}

/// Ends every command followed, as one that went with a turn that was stopped.
pub(crate) fn abandon(shells: &mut Shells) -> Vec<Event> {
    std::mem::take(shells)
        .into_iter()
        .map(|(id, label)| {
            Event::Background(Background {
                id,
                label,
                running: false,
                call: None,
            })
        })
        .collect()
}

/// The command a tool's input names, as the line the reader would have typed.
pub(crate) fn command(input: &Value) -> Option<String> {
    match &input["command"] {
        Value::String(line) => Some(line.clone()),
        Value::Array(words) => {
            let words: Vec<&str> = words.iter().filter_map(Value::as_str).collect();
            (!words.is_empty()).then(|| words.join(" "))
        }
        _ => input["cmd"].as_str().map(str::to_owned),
    }
}

/// Reads a shell handle or its completion from one tool update.
fn background_shell(update: &Value, shells: &mut Shells) -> Option<Event> {
    if !matches!(
        update["sessionUpdate"].as_str(),
        Some("tool_call" | "tool_call_update")
    ) {
        return None;
    }
    let input = &update["rawInput"];
    let output = &update["rawOutput"];
    let id = background_handle(output)
        .or_else(|| background_handle(input).filter(|id| shells.contains_key(id)))?;
    let running = !["exit_code", "exitCode", "exit_status", "exitStatus"]
        .iter()
        .any(|key| !output[*key].is_null())
        && !matches!(update["status"].as_str(), Some("failed" | "cancelled"));
    let label = if running {
        let label = command(input).unwrap_or_else(|| format!("Shell {id}"));
        shells.entry(id.clone()).or_insert(label).clone()
    } else {
        shells.remove(&id)?
    };
    let call = running
        .then(|| update["toolCallId"].as_str().map(str::to_owned))
        .flatten();
    Some(Event::Background(Background {
        id,
        label,
        running,
        call,
    }))
}

/// Reads a background task an agent says it has started, by the result of the call that started it.
fn reported_running(update: &Value, shells: &mut Shells) -> Option<Event> {
    let output = &update["rawOutput"];
    if output["type"].as_str() != Some("BackgroundTaskStarted") {
        return None;
    }
    let id = output["task_id"].as_str()?;
    let label = output["command"]
        .as_str()
        .map_or_else(|| format!("Task {id}"), str::to_owned);
    shells.insert(id.to_owned(), label.clone());
    Some(Event::Background(Background {
        id: id.to_owned(),
        label,
        running: true,
        call: None,
    }))
}

/// Reads the background tasks an agent says are over, from its task list or from the notice of one completing.
fn reported_over(update: &Value, shells: &mut Shells) -> Vec<Event> {
    let mut over = Vec::new();
    if update["sessionUpdate"].as_str() == Some("background_tasks") {
        for task in update["tasks"].as_array().into_iter().flatten() {
            if task["status"].as_str() != Some("running")
                && let Some(id) = task["task_id"].as_str()
            {
                over.push(id.to_owned());
            }
        }
    }
    if update["sessionUpdate"].as_str() == Some("task_completed")
        && let Some(id) = update["task_snapshot"]["task_id"].as_str()
    {
        over.push(id.to_owned());
    }
    over.into_iter()
        .filter_map(|id| {
            let label = shells.remove(&id)?;
            Some(Event::Background(Background {
                id,
                label,
                running: false,
                call: None,
            }))
        })
        .collect()
}

/// Reads the end of a command that outlived its turn from its tool call.
fn settled(update: &Value, shells: &mut Shells) -> Option<Event> {
    if !matches!(
        update["status"].as_str(),
        Some("completed" | "failed" | "cancelled")
    ) {
        return None;
    }
    let id = update["toolCallId"].as_str()?;
    let label = shells.remove(id)?;
    Some(Event::Background(Background {
        id: id.to_owned(),
        label,
        running: false,
        call: None,
    }))
}

/// Reads the start or end of a background task from the lifecycle an agent reports for it.
///
/// A task that is a subagent is left to its card.
fn async_task(update: &Value, shells: &mut Shells) -> Option<Event> {
    let id = update["asyncTaskId"].as_str()?.to_owned();
    match update["sessionUpdate"].as_str()? {
        "async_task_spawned" => {
            if update["taskType"].as_str() == Some("local_agent") {
                return None;
            }
            let label = update["description"]
                .as_str()
                .or_else(|| update["name"].as_str())
                .filter(|label| !label.is_empty())
                .map_or_else(|| format!("Task {id}"), str::to_owned);
            shells.insert(id.clone(), label.clone());
            Some(Event::Background(Background {
                id,
                label,
                running: true,
                call: None,
            }))
        }
        "async_task_state_update" => {
            if matches!(update["state"].as_str(), Some("running" | "paused")) {
                return None;
            }
            let label = shells.remove(&id)?;
            Some(Event::Background(Background {
                id,
                label,
                running: false,
                call: None,
            }))
        }
        _ => None,
    }
}

/// Reads the end of a background task from the notice an agent is woken with, which reaches a window as the task of a subagent started to read it.
fn notified(update: &Value, shells: &mut Shells) -> Option<Event> {
    if update["sessionUpdate"].as_str() != Some("subagent_spawned") {
        return None;
    }
    let notice = update["task"].as_str()?;
    if !notice.contains("<task-notification>") {
        return None;
    }
    let between = |open: &str, close: &str| {
        let start = notice.find(open)? + open.len();
        let end = notice[start..].find(close)? + start;
        Some(notice[start..end].trim())
    };
    if between("<status>", "</status>") == Some("running") {
        return None;
    }
    let id = between("<task-id>", "</task-id>")?.to_owned();
    let label = shells.remove(&id)?;
    Some(Event::Background(Background {
        id,
        label,
        running: false,
        call: None,
    }))
}

/// Reads the process identity a shell tool reports in its input or output.
fn background_handle(value: &Value) -> Option<String> {
    [
        "session_id",
        "sessionId",
        "cell_id",
        "process_id",
        "processId",
    ]
    .iter()
    .find_map(|key| {
        value[*key]
            .as_str()
            .map(str::to_owned)
            .or_else(|| value[*key].as_i64().map(|id| id.to_string()))
    })
}
