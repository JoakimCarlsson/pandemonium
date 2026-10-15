//! The public registry of agents, and what installing one comes to.
//!
//! The Agent Client Protocol keeps a list of agents that speak it, each with
//! how to run it. Only those that are a package a runner fetches are offered;
//! installing one is turning that into the command an agent is started with.

use std::time::Duration;

use serde_json::Value;

use crate::mcp::command_line;

/// Where the registry is read from.
const REGISTRY: &str = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";

/// How long the registry has to answer before it is taken as not answering.
const PATIENCE: Duration = Duration::from_secs(10);

/// The agents to lead with, by their id in the registry; the registry lists
/// them alphabetically and says nothing of which are used.
const FEATURED: [&str; 14] = [
    "claude-acp",
    "codex-acp",
    "gemini",
    "github-copilot-cli",
    "cursor",
    "opencode",
    "goose",
    "kilo",
    "cline",
    "qwen-code",
    "kimi",
    "mistral-vibe",
    "junie",
    "amp-acp",
];

/// One agent the registry offers.
#[derive(Clone, Debug)]
pub struct Available {
    /// What the registry knows it by.
    pub id: String,
    /// What a row calls it.
    pub name: String,
    /// The version the registry lists.
    pub version: String,
    /// What it is, as its publisher says.
    pub description: String,
    /// Where its publisher describes it.
    pub website: String,
    /// The command that runs it, program and arguments.
    pub command: String,
    /// The environment it is started in, over the one it inherits.
    pub env: Vec<(String, String)>,
}

/// The agents the registry offers that can be run on this machine, the
/// well known ones first.
///
/// # Errors
///
/// Says why the registry could not be read.
pub fn fetch() -> Result<Vec<Available>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(PATIENCE))
        .build()
        .into();
    let mut response = agent
        .get(REGISTRY)
        .header("Accept", "application/json")
        .header(
            "User-Agent",
            concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| error.to_string())?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    let registry = serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())?;
    let mut agents = registry["agents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(available)
        .collect::<Vec<_>>();
    agents.sort_by_key(|agent| {
        FEATURED
            .iter()
            .position(|id| *id == agent.id)
            .unwrap_or(FEATURED.len())
    });
    Ok(agents)
}

/// The agent a registry entry stands for, when it can be run here.
fn available(entry: &Value) -> Option<Available> {
    let (command, env) = run(&entry["distribution"])?;
    Some(Available {
        id: entry["id"].as_str()?.to_owned(),
        name: entry["name"].as_str()?.to_owned(),
        version: entry["version"].as_str().unwrap_or_default().to_owned(),
        description: entry["description"].as_str().unwrap_or_default().to_owned(),
        website: entry["website"]
            .as_str()
            .or_else(|| entry["repository"].as_str())
            .unwrap_or_default()
            .to_owned(),
        command,
        env,
    })
}

/// The command that runs an agent from a package, and its environment; an agent that is only an archive to download is left out.
fn run(distribution: &Value) -> Option<(String, Vec<(String, String)>)> {
    for key in ["npx", "uvx"] {
        let Some(package) = distribution[key]["package"].as_str() else {
            continue;
        };
        let mut words = vec![key.to_owned()];
        if key == "npx" {
            words.push("--yes".to_owned());
        }
        words.push(package.to_owned());
        words.extend(strings(&distribution[key]["args"]));
        return Some((
            command_line(words.iter().map(String::as_str)),
            environment(&distribution[key]["env"]),
        ));
    }
    None
}

/// The strings `list` holds.
fn strings(list: &Value) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

/// The variables `table` sets.
fn environment(table: &Value) -> Vec<(String, String)> {
    table
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
        .collect()
}
