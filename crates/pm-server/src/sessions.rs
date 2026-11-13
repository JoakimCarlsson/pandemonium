//! Sessions kept on disk, so that they outlive the server and can be opened again.

use std::io::Write;
use std::path::PathBuf;

use pm_harness::Event;
use serde::{Deserialize, Serialize};

/// The folder under the app's data folder that sessions are kept in.
const FOLDER: &str = "sessions";

/// What a session is, apart from what its agent did.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct Meta {
    /// The agent that runs it, by its id.
    pub(crate) agent: String,
    /// The folder of the project it belongs to, empty for a chat, which belongs to none.
    pub(crate) repository: String,
    /// The folder its agent works in.
    pub(crate) directory: String,
    /// The branch its worktree has, when it was given one of its own.
    pub(crate) branch: Option<String>,
    /// The abbreviated commit its worktree has checked out on a detached head.
    #[serde(default)]
    pub(crate) head: Option<String>,
    /// The branch its worktree was made from, when it has one of its own.
    pub(crate) base: Option<String>,
    /// What it is called.
    pub(crate) title: String,
    /// Whether the reader closed it.
    #[serde(default)]
    pub(crate) closed: bool,
    /// When it last changed, in milliseconds since the epoch.
    pub(crate) updated: u64,
    /// The id of the login its agent runs as, or nothing for the agent's own.
    #[serde(default)]
    pub(crate) login: Option<String>,
    /// The submodule its agent works in, from the root of `directory`, or nothing for the project itself.
    #[serde(default)]
    pub(crate) submodule: Option<String>,
    /// Whether its agent runs in the dev container of `directory`.
    #[serde(default)]
    pub(crate) container: bool,
    /// Whether it is a chat that belongs to no project and works in a scratch folder the server made for it.
    #[serde(default)]
    pub(crate) chat: bool,
    /// The session whose agent started this one, when an agent started it rather than a reader.
    #[serde(default)]
    pub(crate) started_by: Option<u64>,
}

impl Meta {
    /// The folder its agent works in: `directory`, or the submodule's folder inside it.
    pub(crate) fn agent_folder(&self) -> PathBuf {
        let root = PathBuf::from(&self.directory);
        match &self.submodule {
            Some(submodule) => root.join(submodule),
            None => root,
        }
    }
}

/// One line of a session's log: an event and its number.
#[derive(Serialize, Deserialize)]
struct Line {
    /// The event's number within its session.
    seq: u64,
    /// What happened.
    event: Event,
}

/// The folder of the session numbered `number`.
fn folder(number: u64) -> Option<PathBuf> {
    pm_store::data_directory().map(|root| root.join(FOLDER).join(number.to_string()))
}

/// The id the harness keeps a session's saved history under, which makes its file `history.jsonl` in the session's folder.
pub(crate) const HISTORY: &str = "history";

/// The folder the harness keeps the saved history of the session numbered `number` in, which is the session's own folder.
pub(crate) fn history_folder(number: u64) -> Option<PathBuf> {
    folder(number)
}

/// Whether the harness has a saved history for the session numbered `number`.
pub(crate) fn has_history(number: u64) -> bool {
    folder(number).is_some_and(|folder| folder.join(format!("{HISTORY}.jsonl")).is_file())
}

/// Copies the saved history of the session numbered `from` to the session numbered `to`, so that the one continues from where the other was without sharing it afterwards; nothing happens when there is none.
pub(crate) fn copy_history(from: u64, to: u64) {
    let (Some(source), Some(target)) = (folder(from), folder(to)) else {
        return;
    };
    if pm_store::create_private_directory(&target).is_err() {
        return;
    }
    let name = format!("{HISTORY}.jsonl");
    let _ = std::fs::copy(source.join(&name), target.join(&name));
}

/// Writes `meta` as the description of the session numbered `number`.
pub(crate) fn save_meta(number: u64, meta: &Meta) {
    let Some(folder) = folder(number) else {
        return;
    };
    if pm_store::create_private_directory(&folder).is_err() {
        return;
    }
    if let Ok(text) = serde_json::to_string(meta)
        && let Ok(mut file) = pm_store::open_private(&folder.join("meta.json"), false)
    {
        let _ = file.write_all(text.as_bytes());
    }
}

/// What of `event` the log on disk keeps: nothing for what the harness saves with the conversation, and a turn the reader prompted without its words, since only whether its files were kept is the log's to say.
fn on_disk(event: &Event) -> Option<Event> {
    if !crate::conversation::kept_on_disk(event) {
        return None;
    }
    Some(match event {
        Event::Prompted {
            checkpoint,
            turn,
            saved,
            ..
        } => Event::Prompted {
            text: String::new(),
            files: Vec::new(),
            checkpoint: *checkpoint,
            turn: *turn,
            saved: saved.clone(),
        },
        other => other.clone(),
    })
}

/// Hands a session from before the harness kept conversations to the harness, which then holds its conversation, and cuts the copy out of its log on disk; the log as it is when the harness cannot take it, or when there is nothing to hand over.
fn migrate(number: u64, meta: &Meta, log: Vec<(u64, Event)>) -> Vec<(u64, Event)> {
    if has_history(number) {
        return log;
    }
    let (Some(folder), Some(imported)) = (
        history_folder(number),
        crate::conversation::to_import(&log, &meta.title),
    ) else {
        return log;
    };
    if pm_harness::import_saved(HISTORY, &folder, &imported).is_err() {
        return log;
    }
    let kept: Vec<String> = log
        .iter()
        .filter_map(|(seq, event)| {
            serde_json::to_string(&Line {
                seq: *seq,
                event: on_disk(event)?,
            })
            .ok()
        })
        .collect();
    let (path, staged) = (folder.join("events.jsonl"), folder.join("events.jsonl.new"));
    let written = pm_store::open_private(&staged, false)
        .and_then(|mut file| writeln!(file, "{}", kept.join("\n")).map(|()| file.sync_all()))
        .and_then(|synced| synced)
        .and_then(|()| std::fs::rename(&staged, &path));
    if written.is_err() {
        let _ = std::fs::remove_file(&staged);
    }
    log
}

/// Adds `event`, numbered `seq`, to the end of the log of the session numbered `number`, unless the harness saves it with the conversation; a turn the reader prompted is kept without its words, since only whether its files were kept is the log's to say.
pub(crate) fn append(number: u64, seq: u64, event: &Event) {
    let Some(event) = on_disk(event) else {
        return;
    };
    let event = &event;
    let Some(folder) = folder(number) else {
        return;
    };
    if pm_store::create_private_directory(&folder).is_err() {
        return;
    }
    let Ok(line) = serde_json::to_string(&Line {
        seq,
        event: event.clone(),
    }) else {
        return;
    };
    if let Ok(mut file) = pm_store::open_private(&folder.join("events.jsonl"), true) {
        let _ = writeln!(file, "{line}");
    }
}

/// A session kept on disk: its number, its description and its log.
pub(crate) type Stored = (u64, Meta, Vec<(u64, Event)>);

/// `log` and `meta` with the conversation and the name taken from the harness's saved history of the session numbered `number`, when the harness has one it can read; otherwise as they are.
fn from_harness(number: u64, log: Vec<(u64, Event)>, mut meta: Meta) -> (Vec<(u64, Event)>, Meta) {
    if !has_history(number) {
        return (log, meta);
    }
    let Some(folder) = history_folder(number) else {
        return (log, meta);
    };
    match pm_harness::saved_conversation(HISTORY, &folder) {
        Ok(saved) => {
            if let Some(title) = saved.title.clone().filter(|title| !title.is_empty()) {
                meta.title = title;
            }
            (crate::conversation::from_saved(&log, &saved), meta)
        }
        Err(_) => (log, meta),
    }
}

/// Every session kept on disk.
pub(crate) fn load_all() -> Vec<Stored> {
    let Some(root) = pm_store::data_directory().map(|root| root.join(FOLDER)) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<Stored> = entries
        .flatten()
        .filter_map(|entry| {
            let number: u64 = entry.file_name().to_str()?.parse().ok()?;
            let meta: Meta = serde_json::from_str(
                &std::fs::read_to_string(entry.path().join("meta.json")).ok()?,
            )
            .ok()?;
            let log = std::fs::read_to_string(entry.path().join("events.jsonl"))
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str::<Line>(line).ok())
                .map(|line| (line.seq, line.event))
                .collect();
            let log = migrate(number, &meta, log);
            let (log, meta) = from_harness(number, log, meta);
            Some((number, meta, log))
        })
        .collect();
    found.sort_by_key(|(number, _, _)| *number);
    found
}
