//! What an agent's own files say about who it is signed in as and what skills it has installed.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent::Agent;
use crate::product::Product;

/// How deep under an agent's plugin folder skills are looked for.
const PLUGIN_DEPTH: usize = 7;

/// Who `agent` is signed in as, as its own files say: an organisation or an address.
pub fn account(agent: Agent) -> Option<String> {
    account_in(agent, agent.home())
}

/// Who `agent` is signed in as when it keeps its login in its own default folder.
pub fn default_account(agent: Agent) -> Option<String> {
    account_in(agent, None)
}

/// Who `agent` is signed in as in the home folder `own`, or in its default one when there is none.
fn account_in(agent: Agent, own: Option<PathBuf>) -> Option<String> {
    let home = std::env::home_dir()?;
    match agent.product()? {
        Product::Copilot => None,
        Product::Claude => {
            let file = match own {
                Some(own) => own.join(".claude.json"),
                None => home.join(".claude.json"),
            };
            let held: Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
            let account = &held["oauthAccount"];
            account["organizationName"]
                .as_str()
                .or_else(|| account["emailAddress"].as_str())
                .map(str::to_owned)
        }
        Product::Codex => {
            let codex = own
                .or_else(|| std::env::var_os("CODEX_HOME").map(PathBuf::from))
                .unwrap_or_else(|| home.join(".codex"));
            let held: Value =
                serde_json::from_str(&fs::read_to_string(codex.join("auth.json")).ok()?).ok()?;
            let token = held["tokens"]["id_token"].as_str()?;
            let payload = decode(token.split('.').nth(1)?)?;
            let claims: Value = serde_json::from_slice(&payload).ok()?;
            claims["email"].as_str().map(str::to_owned)
        }
        Product::Grok => {
            let grok = own
                .or_else(|| std::env::var_os("GROK_HOME").map(PathBuf::from))
                .unwrap_or_else(|| home.join(".grok"));
            let held: Value =
                serde_json::from_str(&fs::read_to_string(grok.join("auth.json")).ok()?).ok()?;
            held.as_object()?
                .values()
                .find_map(|entry| entry["email"].as_str())
                .map(str::to_owned)
        }
    }
}

/// The names of the skills installed for `agent`, for telling a skill from a command in what it offers behind a slash.
pub fn skill_names(agent: Agent, directory: &Path) -> HashSet<String> {
    let mut names = HashSet::new();
    if agent.product() != Some(Product::Claude) {
        return names;
    }
    if let Some(home) = std::env::home_dir() {
        collect(&home.join(".claude/skills"), 1, &mut names);
        collect(&home.join(".claude/plugins"), PLUGIN_DEPTH, &mut names);
    }
    collect(&directory.join(".claude/skills"), 1, &mut names);
    names
}

/// Adds the name of every folder under `folder`, at most `depth` levels down, that holds a skill.
fn collect(folder: &Path, depth: usize, names: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("SKILL.md").is_file()
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            names.insert(name.to_owned());
        } else if depth > 1 {
            collect(&path, depth - 1, names);
        }
    }
}

/// The bytes a base64 text in the URL-safe alphabet stands for.
fn decode(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for character in text.trim_end_matches('=').chars() {
        let value = match character {
            'A'..='Z' => character as u32 - 'A' as u32,
            'a'..='z' => character as u32 - 'a' as u32 + 26,
            '0'..='9' => character as u32 - '0' as u32 + 52,
            '-' | '+' => 62,
            '_' | '/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(bytes)
}
