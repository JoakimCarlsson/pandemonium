//! What an agent's own files say about who it is signed in as.

use std::fs;
use std::path::PathBuf;

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

/// Who `agent` is signed in as in the account folder `home`, whichever folder is in use.
pub fn account_in_folder(agent: Agent, home: PathBuf) -> Option<String> {
    account_in(agent, Some(home))
}

/// Who `agent` is signed in as in the home folder `own`, or in its default one when there is none.
fn account_in(agent: Agent, own: Option<PathBuf>) -> Option<String> {
    let home = std::env::home_dir()?;
    match agent.product()? {
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
