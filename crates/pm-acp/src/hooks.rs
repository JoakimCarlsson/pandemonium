//! The hooks Codex holds back until the reader trusts them: finding them and trusting them.
//!
//! Both are asked of Codex's own app server, started apart from the agent's for
//! the one exchange, so the trust recorded is the one Codex computes and
//! `codex` in a terminal sees the same state.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use crate::agent::{Agent, RUNNER, Source};
use crate::chat::Event;
use crate::client::{Answer, Client};
use pm_tools::{installed, path_beside};

/// How long Codex's app server has to start and answer, when it is installed.
const QUICK: Duration = Duration::from_secs(30);

/// How long Codex's app server has to start and answer, when the runner has to fetch it first.
const FETCH: Duration = Duration::from_secs(180);

/// How often the files hooks are read from are looked at for a change.
const LOOK: Duration = Duration::from_secs(2);

/// One hook Codex will not run until the reader trusts it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Hook {
    /// What Codex keeps the hook's trust under.
    pub key: String,
    /// The event it runs on, as Codex names it, such as `preToolUse`.
    pub event: String,
    /// What the event must match for the hook to run, when it says.
    pub matcher: Option<String>,
    /// The command it runs, or what else it calls on.
    pub command: String,
    /// The file it is read from.
    pub source: String,
    /// What Codex makes of the hook as it stands, which trusting it records.
    pub hash: String,
    /// Whether it was trusted before and changed since, rather than new.
    pub changed: bool,
}

/// Codex's app server, started with the program and config folder `agent` would use.
struct AppServer {
    /// The process, spoken to line by line.
    client: Client,
    /// How long it is given to answer.
    patience: Duration,
    /// The folder Codex says it keeps its config in.
    home: PathBuf,
}

/// The `codex` program the agent runs, found as the agent finds it: the one it is pointed at, the one bundled beside it, or the one the runner fetches with it.
fn codex(agent: Agent) -> (Command, bool) {
    let pointed = agent
        .env
        .iter()
        .find(|(name, _)| *name == "CODEX_PATH")
        .map(|(_, value)| PathBuf::from(value))
        .or_else(|| std::env::var_os("CODEX_PATH").map(PathBuf::from));
    let program = installed(agent.program);
    let bundled = program
        .as_deref()
        .and_then(|program| std::fs::canonicalize(program).ok())
        .and_then(|script| bundled_codex(&script));
    match (pointed.or(bundled), program, agent.source) {
        (Some(codex), _, _) => (located(codex), false),
        (None, None, Source::Package(package)) => {
            let runner = installed(RUNNER);
            let mut command = Command::new(runner.clone().unwrap_or_else(|| RUNNER.into()));
            if let Some(runner) = runner.as_deref() {
                command.env("PATH", path_beside(runner));
            }
            command.args(["--yes", "--package", package, "codex"]);
            (command, true)
        }
        (None, _, _) => (
            located(installed("codex").unwrap_or_else(|| "codex".into())),
            false,
        ),
    }
}

/// The command that runs the program at `program`, with the folder holding it first on the path.
fn located(program: PathBuf) -> Command {
    let mut command = Command::new(&program);
    command.env("PATH", path_beside(&program));
    command
}

/// The `codex` a package installed with `script`, its entry point, depends on: in its own `node_modules` or beside it in the one above.
fn bundled_codex(script: &Path) -> Option<PathBuf> {
    let package = script.parent()?.parent()?;
    let names: &[&str] = if cfg!(windows) {
        &["codex.cmd", "codex"]
    } else {
        &["codex"]
    };
    [
        Some(package.join("node_modules/.bin")),
        package
            .parent()?
            .parent()
            .map(|modules| modules.join(".bin")),
    ]
    .into_iter()
    .flatten()
    .flat_map(|folder| names.iter().map(move |name| folder.join(name)))
    .find(|candidate| candidate.is_file())
}

impl AppServer {
    /// Starts the app server of the Codex `agent` runs in `directory`, keeping its config in `home` or in its own folder when there is none, and introduces the app to it.
    fn start(agent: Agent, directory: &Path, home: Option<&Path>) -> Result<Self, String> {
        let (mut command, fetches) = codex(agent);
        command.arg("app-server").envs(agent.env.iter().copied());
        if let Some(home) = home {
            command.env(agent.home.unwrap_or("CODEX_HOME"), home);
        }
        let patience = if fetches { FETCH } else { QUICK };
        let mut client =
            Client::start_in(command, directory).map_err(|reason| format!("Codex {reason}"))?;
        let shaken = result(client.ask(
            "initialize",
            &json!({ "clientInfo": { "name": "pandemonium", "title": "Pandemonium", "version": env!("CARGO_PKG_VERSION") } }),
            patience,
        )?)?;
        client.send(&json!({ "jsonrpc": "2.0", "method": "initialized" }))?;
        Ok(Self {
            client,
            patience,
            home: shaken["codexHome"]
                .as_str()
                .map(PathBuf::from)
                .unwrap_or_default(),
        })
    }

    /// Every hook Codex reads for work in `directory`, as it lists them.
    fn hooks(&mut self, directory: &Path) -> Result<Vec<Value>, String> {
        let listed = result(self.client.ask(
            "hooks/list",
            &json!({ "cwds": [directory] }),
            self.patience,
        )?)?;
        Ok(listed["data"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|entry| entry["hooks"].as_array().cloned().unwrap_or_default())
            .collect())
    }

    /// Records each of `hooks` as trusted as it stands, the way Codex records it itself.
    fn trust(&mut self, hooks: &[Hook]) -> Result<(), String> {
        let trusted: serde_json::Map<String, Value> = hooks
            .iter()
            .map(|hook| (hook.key.clone(), json!({ "trusted_hash": hook.hash })))
            .collect();
        result(self.client.ask(
            "config/batchWrite",
            &json!({
                "edits": [{ "keyPath": "hooks.state", "value": trusted, "mergeStrategy": "upsert" }],
                "reloadUserConfig": true,
            }),
            self.patience,
        )?)
        .map(|_| ())
    }
}

/// The result of `answer`, or the reason it failed.
fn result(answer: Answer) -> Result<Value, String> {
    match answer {
        Answer::Result(result) => Ok(result),
        Answer::Error(_, message) => Err(message),
    }
}

/// The hook `listed` describes, when Codex holds it back until it is trusted.
fn untrusted(listed: &Value) -> Option<Hook> {
    let changed = match listed["trustStatus"].as_str()? {
        "untrusted" => false,
        "modified" => true,
        _ => return None,
    };
    let command = match listed["handlerType"].as_str() {
        Some("command") => listed["command"].as_str().unwrap_or_default().to_owned(),
        Some("mcpTool") => format!(
            "{} {}",
            listed["server"].as_str().unwrap_or_default(),
            listed["tool"].as_str().unwrap_or_default()
        ),
        other => other.unwrap_or_default().to_owned(),
    };
    Some(Hook {
        key: listed["key"].as_str()?.to_owned(),
        event: listed["eventName"].as_str().unwrap_or_default().to_owned(),
        matcher: listed["matcher"].as_str().map(str::to_owned),
        command,
        source: listed["sourcePath"].as_str().unwrap_or_default().to_owned(),
        hash: listed["currentHash"].as_str()?.to_owned(),
        changed,
    })
}

/// The hooks Codex holds back from the agent `agent` working in `directory`, with its config in `home` or in its own folder when there is none, and the files a change to which may change that.
fn review(
    agent: Agent,
    directory: &Path,
    home: Option<&Path>,
) -> Result<(Vec<Hook>, Vec<PathBuf>), String> {
    let mut server = AppServer::start(agent, directory, home)?;
    let listed = server.hooks(directory)?;
    let mut files: Vec<PathBuf> = [
        server.home.join("hooks.json"),
        server.home.join("config.toml"),
        directory.join(".codex/hooks.json"),
        directory.join(".codex/config.toml"),
    ]
    .into();
    for source in listed.iter().filter_map(|hook| hook["sourcePath"].as_str()) {
        let source = PathBuf::from(source);
        if !files.contains(&source) {
            files.push(source);
        }
    }
    Ok((listed.iter().filter_map(untrusted).collect(), files))
}

/// Trusts those of `hooks` Codex still holds back from the agent `agent` working in `directory` as they were shown, with its config in `home` or in its own folder when there is none, and returns the hooks it holds back after that.
///
/// A hook that changed since it was shown is left untrusted, so nothing is trusted that the reader did not see.
pub fn trust_hooks(
    agent: Agent,
    directory: &Path,
    home: Option<&Path>,
    hooks: &[Hook],
) -> Result<Vec<Hook>, String> {
    let mut server = AppServer::start(agent, directory, home)?;
    let held: Vec<Hook> = server
        .hooks(directory)?
        .iter()
        .filter_map(untrusted)
        .collect();
    let shown: Vec<Hook> = held
        .iter()
        .filter(|hook| {
            hooks
                .iter()
                .any(|chosen| chosen.key == hook.key && chosen.hash == hook.hash)
        })
        .cloned()
        .collect();
    if !shown.is_empty() {
        server.trust(&shown)?;
    }
    Ok(server
        .hooks(directory)?
        .iter()
        .filter_map(untrusted)
        .collect())
}

/// When each of `files` was last changed, or nothing for one that is not there.
fn stamps(files: &[PathBuf]) -> Vec<Option<SystemTime>> {
    files
        .iter()
        .map(|file| {
            std::fs::metadata(file)
                .and_then(|held| held.modified())
                .ok()
        })
        .collect()
}

/// Tells `events` the hooks Codex holds back from the agent `agent` working in `directory`, with its config in `home` or in its own folder when there is none, and tells it again each time a file they are read from changes, until `stop` is dropped.
pub(crate) fn watch(
    agent: Agent,
    directory: PathBuf,
    home: Option<PathBuf>,
    events: Sender<Event>,
    stop: Receiver<()>,
) {
    let mut files: Vec<PathBuf> = Vec::new();
    loop {
        if let Ok((held, read_from)) = review(agent, &directory, home.as_deref()) {
            if events.send(Event::Hooks(held)).is_err() {
                return;
            }
            files = read_from;
        }
        let seen = stamps(&files);
        loop {
            if !matches!(stop.recv_timeout(LOOK), Err(RecvTimeoutError::Timeout)) {
                return;
            }
            if stamps(&files) != seen {
                break;
            }
        }
    }
}
