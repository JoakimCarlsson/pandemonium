//! The public registry of MCP servers, and what installing one comes to.
//!
//! A listing in the registry says how a server is run: as a package a
//! command fetches and starts, or as an address it listens at. Installing
//! one is turning that into the [`McpServer`] an agent is opened with, and
//! saying which of its variables the reader has to fill in first.

use std::time::Duration;

use serde_json::Value;

use crate::mcp::{McpServer, Reach};

/// Where the registry is asked for servers.
const REGISTRY: &str = "https://registry.modelcontextprotocol.io/v0.1/servers";

/// How many servers one search asks for.
const PAGE: usize = 50;

/// How long the registry has to answer before it is taken as not answering.
const PATIENCE: Duration = Duration::from_secs(10);

/// One server the registry offers.
#[derive(Clone, Debug)]
pub struct Listing {
    /// What the server is called in the registry.
    pub id: String,
    /// What a row calls it.
    pub title: String,
    /// What it does, as its publisher says.
    pub description: String,
    /// The server as installed, its variables still to be filled in.
    pub server: McpServer,
    /// What the reader has to give it before it will run.
    pub required: Vec<String>,
}

/// The servers whose names or descriptions match `query`, or the first page
/// of the registry when it is blank.
///
/// # Errors
///
/// Says why the registry could not be read.
pub fn search(query: &str) -> Result<Vec<Listing>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(PATIENCE))
        .build()
        .into();
    let mut request = agent
        .get(REGISTRY)
        .query("limit", PAGE.to_string())
        .query("version", "latest")
        .header("Accept", "application/json")
        .header(
            "User-Agent",
            concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
        );
    if !query.trim().is_empty() {
        request = request.query("search", query.trim());
    }
    let mut response = request.call().map_err(|error| error.to_string())?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    let page = serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())?;
    Ok(page["servers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| listing(&entry["server"]))
        .collect())
}

/// The listing a registry entry stands for, when it can be run from here.
fn listing(entry: &Value) -> Option<Listing> {
    let id = entry["name"].as_str()?.to_owned();
    let title = id.rsplit('/').next().unwrap_or(&id).to_owned();
    let (reach, required) = remote(entry).or_else(|| package(entry))?;
    Some(Listing {
        description: entry["description"].as_str().unwrap_or_default().to_owned(),
        server: McpServer {
            name: title.clone(),
            reach,
        },
        title,
        id,
        required,
    })
}

/// How the first remote the entry offers is reached, and what it asks for.
fn remote(entry: &Value) -> Option<(Reach, Vec<String>)> {
    let remote = entry["remotes"]
        .as_array()?
        .iter()
        .find(|remote| matches!(remote["type"].as_str(), Some("streamable-http" | "sse")))?;
    let url = remote["url"].as_str()?.to_owned();
    let (headers, required) = variables(&remote["headers"]);
    let reach = match remote["type"].as_str() {
        Some("sse") => Reach::Events { url, headers },
        _ => Reach::Http { url, headers },
    };
    Some((reach, required))
}

/// How the first package the entry offers is started, and what it asks for.
fn package(entry: &Value) -> Option<(Reach, Vec<String>)> {
    let package = entry["packages"].as_array()?.iter().find(|package| {
        matches!(package["transport"]["type"].as_str(), None | Some("stdio"))
            && matches!(
                package["registryType"].as_str(),
                Some("npm" | "pypi" | "oci")
            )
    })?;
    let identifier = package["identifier"].as_str()?;
    let version = package["version"]
        .as_str()
        .filter(|version| !version.is_empty());
    let (program, mut arguments) = match package["registryType"].as_str()? {
        "npm" => (
            package["runtimeHint"].as_str().unwrap_or("npx").to_owned(),
            vec![
                "-y".to_owned(),
                match version {
                    Some(version) => format!("{identifier}@{version}"),
                    None => identifier.to_owned(),
                },
            ],
        ),
        "pypi" => (
            package["runtimeHint"].as_str().unwrap_or("uvx").to_owned(),
            vec![identifier.to_owned()],
        ),
        _ => (
            "docker".to_owned(),
            vec![
                "run".to_owned(),
                "-i".to_owned(),
                "--rm".to_owned(),
                identifier.to_owned(),
            ],
        ),
    };
    arguments.extend(
        package["packageArguments"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(argument),
    );
    let (env, required) = variables(&package["environmentVariables"]);
    Some((
        Reach::Command {
            program,
            arguments,
            env,
        },
        required,
    ))
}

/// The words one argument of a package is written as on a command line.
fn argument(argument: &Value) -> Vec<String> {
    let value = argument["value"].as_str().map(str::to_owned);
    match argument["type"].as_str() {
        Some("named") => argument["name"]
            .as_str()
            .map(str::to_owned)
            .into_iter()
            .chain(value)
            .collect(),
        _ => value.into_iter().collect(),
    }
}

/// The names and starting values of the variables or headers an entry lists,
/// and which of them the reader has to fill in.
fn variables(list: &Value) -> (Vec<(String, String)>, Vec<String>) {
    let mut pairs = Vec::new();
    let mut required = Vec::new();
    for variable in list.as_array().into_iter().flatten() {
        let Some(name) = variable["name"].as_str() else {
            continue;
        };
        let value = variable["value"]
            .as_str()
            .or_else(|| variable["default"].as_str())
            .unwrap_or_default();
        if variable["isRequired"] == true || value.contains('{') {
            required.push(name.to_owned());
        }
        if variable["isRequired"] == true || !value.is_empty() {
            pairs.push((name.to_owned(), value.to_owned()));
        }
    }
    (pairs, required)
}
