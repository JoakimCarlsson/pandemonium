//! Sessions kept on disk, so that they outlive the server and can be opened again.

use std::io::Write;
use std::path::PathBuf;

use pm_acp::Event;
use serde::{Deserialize, Serialize};

/// The folder under the app's data folder that sessions are kept in.
const FOLDER: &str = "sessions";

/// What a session is, apart from what its agent did.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct Meta {
    /// The agent that runs it, by its id.
    pub(crate) agent: String,
    /// The folder of the project it belongs to.
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

/// Adds `event`, numbered `seq`, to the end of the log of the session numbered `number`.
pub(crate) fn append(number: u64, seq: u64, event: &Event) {
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
            Some((number, meta, log))
        })
        .collect();
    found.sort_by_key(|(number, _, _)| *number);
    found
}
