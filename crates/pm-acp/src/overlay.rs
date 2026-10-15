//! A home folder of its own for an agent that has no variable to move its login folder: the reader's home folder with the agent's own folders left out, so the agent finds its login, and its login alone, in the account's folder.

use std::fs;
use std::path::Path;

use crate::agent::Agent;

/// Words that name how an agent is run or what it is, not the folder it keeps its config in.
const COMMON: [&str; 16] = [
    "npx",
    "yes",
    "acp",
    "agent",
    "agents",
    "cli",
    "code",
    "build",
    "github",
    "stdio",
    "node",
    "app",
    "bin",
    "run",
    "agentclientprotocol",
    "modelcontextprotocol",
];

/// What an agent's own folder holds that an account folder shares with it, so that a new account has the skills and rules the reader already has.
const SHARED: [&str; 9] = [
    "AGENTS.md",
    "skills",
    "agents",
    "commands",
    "rules",
    "hooks",
    "plugins",
    "installed-plugins",
    "instructions",
];

/// The folders of the home folder whose children are linked one by one, because other programs keep theirs among an agent's.
const CONTAINERS: [&str; 5] = [
    ".config",
    ".cache",
    ".local",
    ".local/share",
    ".local/state",
];

/// The words in what runs `agent` that its own folders are likely named after, in lower case.
fn keywords(agent: Agent) -> Vec<String> {
    let package = match agent.source {
        crate::agent::Source::Package(package) => package,
        _ => "",
    };
    [agent.id, agent.name, agent.program, package]
        .into_iter()
        .chain(agent.arguments.iter().copied())
        .flat_map(|text| text.split(|part: char| !part.is_alphanumeric()))
        .map(str::to_lowercase)
        .filter(|word| word.len() >= 3 && !COMMON.contains(&word.as_str()))
        .collect()
}

/// Whether the folder or file called `name` is one the agent keeps its own things in.
fn belongs_to(name: &str, words: &[String]) -> bool {
    let name = name.trim_start_matches('.').to_lowercase();
    words.iter().any(|word| name.starts_with(word.as_str()))
}

/// Links the entries of `real` into `overlay` except the agent's own, one level down in the folders that hold other programs' folders, and shares the agent's skills and rules with its own folders in `overlay`.
fn link(real: &Path, overlay: &Path, relative: &str, words: &[String]) {
    let Ok(entries) = fs::read_dir(real) else {
        return;
    };
    let _ = fs::create_dir_all(overlay);
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let here = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        let target = overlay.join(&name);
        if belongs_to(&name, words) {
            share(&path, &target);
        } else if CONTAINERS.contains(&here.as_str()) && path.is_dir() {
            link(&path, &target, &here, words);
        } else if target.symlink_metadata().is_err() {
            symlink(&path, &target);
        }
    }
}

/// Links what `own`, an agent's own folder, shares into `target`, the same folder in an account's home.
fn share(own: &Path, target: &Path) {
    if !own.is_dir() {
        return;
    }
    for name in SHARED {
        let from = own.join(name);
        let to = target.join(name);
        if from.exists() && to.symlink_metadata().is_err() {
            let _ = fs::create_dir_all(target);
            symlink(&from, &to);
        }
    }
}

/// Makes `to` a link to `from`.
fn symlink(from: &Path, to: &Path) {
    #[cfg(unix)]
    let _ = std::os::unix::fs::symlink(from, to);
}

/// Fills `overlay` as the home folder an account of `agent` runs it in.
pub(crate) fn build(agent: Agent, overlay: &Path) {
    if let Some(real) = std::env::home_dir() {
        link(&real, overlay, "", &keywords(agent));
    }
}
