//! What an agent's own files say about who it is signed in as and what skills it has installed.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent::Agent;
use crate::product::Product;

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

/// The skills `agent` is given in the project at `directory`: those found in the folders it keeps them in, less the ones `withheld` turns off.
pub(crate) fn given_skills(
    agent: Agent,
    directory: &Path,
    withheld: &[String],
) -> Vec<pm_agent_library::Item> {
    pm_agent_library::skills(agent.product(), directory, agent.config_folder().as_deref())
        .into_iter()
        .filter(|skill| skill.enabled(withheld))
        .collect()
}

/// The names of the skills installed for `agent` and not turned off in `withheld`, for telling a skill from a command in what it offers behind a slash; a skill a plugin brings is named without its plugin.
pub fn skill_names(agent: Agent, directory: &Path, withheld: &[String]) -> HashSet<String> {
    if agent.product() != Some(Product::Claude) {
        return HashSet::new();
    }
    given_skills(agent, directory, withheld)
        .into_iter()
        .map(|skill| skill.name.rsplit(':').next().unwrap_or_default().to_owned())
        .collect()
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
