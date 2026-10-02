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

/// How many servers one search asks the registry for, to rank among.
const PAGE: usize = 100;

/// How many of the ranked servers a search keeps.
const SHOWN: usize = 50;

/// The servers worth offering before anything is searched for, by the name
/// the registry knows them by. The registry lists in alphabetical order of
/// name and says nothing of which servers are used, so what to lead with
/// is chosen here rather than left to the alphabet.
const FEATURED: [&str; 15] = [
    "io.github.upstash/context7",
    "io.github.github/github-mcp-server",
    "io.github.microsoft/playwright-mcp",
    "io.github.ChromeDevTools/chrome-devtools-mcp",
    "io.github.oraios/serena",
    "io.github.brave/brave-search-mcp-server",
    "com.notion/mcp",
    "app.linear/linear",
    "com.figma.mcp/mcp",
    "com.stripe/mcp",
    "com.cloudflare.mcp/mcp",
    "com.supabase/mcp",
    "com.atlassian/atlassian-mcp-server",
    "io.github.hashicorp/terraform-mcp-server",
    "io.github.mongodb-js/mongodb-mcp-server",
];

/// Namespaces that publish proxies and tunnels rather than servers of their own.
const NOISY: [&str; 3] = ["ai.smithery/", "com.trycloudflare.", "app.vercel."];

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
    /// What the server lists as taking, a key or a token for instance,
    /// whether or not it will run without.
    pub inputs: Vec<String>,
    /// Whether installing it has to ask for something first: a listed input
    /// is required, or is a secret such as a key.
    pub asks: bool,
}

/// The servers that match `query`, best match first, or the featured ones
/// when it is blank.
///
/// # Errors
///
/// Says why the registry could not be read.
pub fn search(query: &str) -> Result<Vec<Listing>, String> {
    let query = query.trim();
    if query.is_empty() {
        return featured();
    }
    let page = get(
        REGISTRY,
        &[
            ("limit", PAGE.to_string()),
            ("version", "latest".to_owned()),
            ("search", query.to_owned()),
        ],
    )?;
    let mut ranked = page["servers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| listing(&entry["server"]))
        .map(|listing| (score(query, &listing), listing))
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    Ok(ranked
        .into_iter()
        .map(|(_, listing)| listing)
        .take(SHOWN)
        .collect())
}

/// The featured servers, in the order they are listed above.
///
/// A server the registry no longer has is left out; the registry not
/// answering at all is the only failure.
fn featured() -> Result<Vec<Listing>, String> {
    let answers = std::thread::scope(|scope| {
        let asked = FEATURED
            .iter()
            .map(|name| scope.spawn(move || latest(name)))
            .collect::<Vec<_>>();
        asked
            .into_iter()
            .map(|asked| asked.join().unwrap_or(Err("the search stopped".to_owned())))
            .collect::<Vec<_>>()
    });
    let listings = answers
        .iter()
        .filter_map(|answer| answer.as_ref().ok())
        .filter_map(|entry| listing(&entry["server"]))
        .collect::<Vec<_>>();
    match (
        listings.is_empty(),
        answers.into_iter().find_map(Result::err),
    ) {
        (true, Some(error)) => Err(error),
        _ => Ok(listings),
    }
}

/// What the registry says of the newest version of the server `name`.
fn latest(name: &str) -> Result<Value, String> {
    let name = name.replace('/', "%2F");
    get(&format!("{REGISTRY}/{name}/versions/latest"), &[])
}

/// What the registry answers a request for `url` with.
fn get(url: &str, query: &[(&str, String)]) -> Result<Value, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(PATIENCE))
        .build()
        .into();
    let request = query.iter().fold(
        agent.get(url).header("Accept", "application/json").header(
            "User-Agent",
            concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
        ),
        |request, (name, value)| request.query(*name, value.as_str()),
    );
    let mut response = request.call().map_err(|error| error.to_string())?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

/// How well `listing` answers `query`: what it is called counts most, then
/// whether its publisher is the one the name suggests, and last whether it
/// is a proxy or a tunnel that stands in for a server.
fn score(query: &str, listing: &Listing) -> i32 {
    let query = query.to_lowercase();
    let id = listing.id.to_lowercase();
    let slug = listing.server.name.to_lowercase();
    let title = listing.title.to_lowercase();
    let description = listing.description.to_lowercase();
    let words = query.split_whitespace().collect::<Vec<_>>();
    let name = if slug == query || title == query {
        100
    } else if slug.starts_with(&query) || title.starts_with(&query) {
        70
    } else if slug.contains(&query) || title.contains(&query) {
        50
    } else if words
        .iter()
        .all(|word| id.contains(word) || description.contains(word))
    {
        20
    } else {
        0
    };
    let namespace = id.split('/').next().unwrap_or_default();
    let publisher = match namespace.strip_prefix("io.github.") {
        Some(owner) if slug.contains(owner) || words.iter().any(|word| owner.contains(word)) => 25,
        Some(_) => 5,
        None => {
            let first_party = namespace
                .split('.')
                .skip(1)
                .any(|part| words.iter().any(|word| part.contains(word)) || slug.contains(part));
            match first_party {
                true => 40,
                false => 10,
            }
        }
    };
    let featured = match FEATURED
        .iter()
        .any(|name| name.eq_ignore_ascii_case(&listing.id))
    {
        true => 80,
        false => 0,
    };
    let noisy = match NOISY.iter().any(|prefix| id.starts_with(prefix)) {
        true => -60,
        false => 0,
    };
    let bare = match listing.description.trim().is_empty() {
        true => -10,
        false => 0,
    };
    name + publisher + featured + noisy + bare
}

/// The listing a registry entry stands for, when it can be run from here.
fn listing(entry: &Value) -> Option<Listing> {
    let id = entry["name"].as_str()?.to_owned();
    let slug = short_name(&id);
    let title = entry["title"]
        .as_str()
        .filter(|title| !title.is_empty())
        .unwrap_or(&slug)
        .to_owned();
    let (reach, inputs) = remote(entry).or_else(|| package(entry))?;
    let description = entry["description"].as_str().unwrap_or_default().to_owned();
    Some(Listing {
        description: description.clone(),
        server: McpServer {
            name: slug,
            reach,
            description,
            website: entry["websiteUrl"]
                .as_str()
                .or_else(|| entry["repository"]["url"].as_str())
                .unwrap_or_default()
                .to_owned(),
            enabled: true,
        },
        title,
        id,
        inputs: inputs.names,
        asks: inputs.asks,
    })
}

/// What a server is called when it is installed, from the name the registry
/// knows it by: the part after the slash, or the publisher's own name when
/// that part says nothing but what kind of server it is.
fn short_name(id: &str) -> String {
    let (namespace, name) = id.split_once('/').unwrap_or(("", id));
    match name {
        "mcp" | "server" | "mcp-server" => namespace
            .split('.')
            .skip(1)
            .find(|label| !matches!(*label, "mcp" | "io" | "github" | "app"))
            .unwrap_or(name)
            .to_owned(),
        _ => name.to_owned(),
    }
}

/// What an entry lists as taking.
struct Inputs {
    /// Their names, in the order listed.
    names: Vec<String>,
    /// Whether any is required or secret.
    asks: bool,
}

/// How the first remote the entry offers is reached, and what it asks for.
fn remote(entry: &Value) -> Option<(Reach, Inputs)> {
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
fn package(entry: &Value) -> Option<(Reach, Inputs)> {
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

/// The names and starting values of the variables or headers an entry
/// lists, and what asking for them comes to.
///
/// A key that is not required is usually still what lifts a limit, so a
/// secret is asked for like a required input is; any other input that is
/// optional is left for the reader to add later.
fn variables(list: &Value) -> (Vec<(String, String)>, Inputs) {
    let mut pairs = Vec::new();
    let mut inputs = Inputs {
        names: Vec::new(),
        asks: false,
    };
    for variable in list.as_array().into_iter().flatten() {
        let Some(name) = variable["name"].as_str() else {
            continue;
        };
        let value = variable["value"]
            .as_str()
            .or_else(|| variable["default"].as_str())
            .unwrap_or_default();
        inputs.asks |=
            variable["isRequired"] == true || variable["isSecret"] == true || value.contains('{');
        inputs.names.push(name.to_owned());
        pairs.push((name.to_owned(), value.to_owned()));
    }
    (pairs, inputs)
}
