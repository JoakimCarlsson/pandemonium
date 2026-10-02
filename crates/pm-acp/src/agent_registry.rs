//! The public registry of agents, and what installing one comes to.
//!
//! The Agent Client Protocol keeps a list of agents that speak it, each with
//! how to run it: as a package a runner fetches, or as an archive to be
//! downloaded for the platform at hand. Installing one is turning that into
//! the command an agent is started with.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::agent::{Agent, Source};

/// Where the registry is read from.
const REGISTRY: &str = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";

/// How long the registry has to answer before it is taken as not answering.
const PATIENCE: Duration = Duration::from_secs(10);

/// How long an archive has to arrive before it is taken as not coming.
const DOWNLOAD_PATIENCE: Duration = Duration::from_secs(300);

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
    /// How it comes to be run.
    pub install: Install,
}

impl Available {
    /// Whether `agent` is this agent: it goes by the same id or name, or it
    /// is started from the same package, which is how the editor's own
    /// Claude Code and the registry's Claude Agent turn out to be one.
    #[must_use]
    pub fn is(&self, agent: &Agent) -> bool {
        if self.id == agent.id || self.name.eq_ignore_ascii_case(agent.name) {
            return true;
        }
        let Source::Package(shipped) = agent.source else {
            return false;
        };
        match &self.install {
            Install::Run { arguments, .. } => arguments
                .iter()
                .any(|argument| unversioned(argument) == shipped),
            Install::Download(_) => false,
        }
    }
}

/// `package` without the version a registry pins it to.
fn unversioned(package: &str) -> &str {
    match package.rfind('@') {
        Some(at) if at > 0 => &package[..at],
        _ => package,
    }
}

/// How an agent comes to be run.
#[derive(Clone, Debug)]
pub enum Install {
    /// A command that fetches and runs a package.
    Run {
        /// The program to run.
        program: String,
        /// The arguments to run it with.
        arguments: Vec<String>,
        /// The environment to start it in, over the one it inherits.
        env: Vec<(String, String)>,
    },
    /// An archive to download and unpack, holding the program.
    Download(Download),
}

/// An archive that holds an agent's program, for one platform.
#[derive(Clone, Debug)]
pub struct Download {
    /// Where the archive is.
    pub archive: String,
    /// What the archive hashes to, when the registry says.
    pub sha256: Option<String>,
    /// The program inside it, relative to where it is unpacked.
    pub command: String,
    /// The arguments to run it with.
    pub arguments: Vec<String>,
    /// The environment to start it in, over the one it inherits.
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
        install: install(&entry["distribution"])?,
    })
}

/// How an agent is run, preferring what needs nothing downloaded here.
fn install(distribution: &Value) -> Option<Install> {
    for (runner, key) in [("npx", "npx"), ("uvx", "uvx")] {
        let Some(package) = distribution[key]["package"].as_str() else {
            continue;
        };
        let mut arguments = match key {
            "npx" => vec!["--yes".to_owned()],
            _ => Vec::new(),
        };
        arguments.push(package.to_owned());
        arguments.extend(strings(&distribution[key]["args"]));
        return Some(Install::Run {
            program: runner.to_owned(),
            arguments,
            env: environment(&distribution[key]["env"]),
        });
    }
    let entry = &distribution["binary"][platform()?];
    Some(Install::Download(Download {
        archive: entry["archive"].as_str()?.to_owned(),
        sha256: entry["sha256"].as_str().map(str::to_lowercase),
        command: entry["cmd"].as_str()?.to_owned(),
        arguments: strings(&entry["args"]),
        env: environment(&entry["env"]),
    }))
}

/// What the registry calls the platform this runs on.
fn platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x86_64"),
        ("linux", "aarch64") => Some("linux-aarch64"),
        ("macos", "x86_64") => Some("darwin-x86_64"),
        ("macos", "aarch64") => Some("darwin-aarch64"),
        ("windows", "x86_64") => Some("windows-x86_64"),
        ("windows", "aarch64") => Some("windows-aarch64"),
        _ => None,
    }
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

/// Downloads and unpacks `download` into `into`, and says where the program
/// it holds now is.
///
/// An archive that does not hash to what the registry says is thrown away
/// rather than unpacked.
///
/// # Errors
///
/// Says what went wrong: the download, the hash, or the unpacking.
pub fn download(download: &Download, into: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(into).map_err(|error| error.to_string())?;
    let archive = into.join(
        download
            .archive
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("agent-archive"),
    );
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(DOWNLOAD_PATIENCE))
        .build()
        .into();
    let mut response = agent
        .get(&download.archive)
        .header(
            "User-Agent",
            concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if let Some(expected) = &download.sha256 {
        let found = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if &found != expected {
            return Err("the download is not what the registry lists".to_owned());
        }
    }
    fs::write(&archive, &bytes).map_err(|error| error.to_string())?;
    unpack(&archive, into)?;
    let _ = fs::remove_file(&archive);
    let program = into.join(download.command.trim_start_matches("./"));
    if !program.exists() {
        return Err(format!("{} is not in the download", download.command));
    }
    make_runnable(&program)?;
    Ok(program)
}

/// Unpacks `archive` into `into`, with the tool the platform has for its kind.
fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    let name = archive.to_string_lossy().to_lowercase();
    let mut command = match name.ends_with(".zip") && cfg!(not(windows)) {
        true => {
            let mut command = Command::new("unzip");
            command.args(["-o", "-q"]).arg(archive).arg("-d").arg(into);
            command
        }
        false => {
            let mut command = Command::new("tar");
            command.arg("-xf").arg(archive).arg("-C").arg(into);
            command
        }
    };
    let status = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    match status.success() {
        true => Ok(()),
        false => Err("the download could not be unpacked".to_owned()),
    }
}

/// Lets `program` be run.
fn make_runnable(program: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(program, fs::Permissions::from_mode(0o755))
            .map_err(|error| error.to_string())?;
    }
    #[cfg(not(unix))]
    let _ = program;
    Ok(())
}
