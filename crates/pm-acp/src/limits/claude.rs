//! Claude Code's limits, as the service its own `/usage` reads reports them.
//!
//! The adapter passes a rate-limit event on only when the limit's standing
//! changes, which an ordinary turn never does, so the windows are read where
//! Claude Code reads them: the usage service, with the token Claude Code
//! keeps once it is logged in. The service is undocumented and may change
//! without notice.

use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use super::{Limits, Window, clock, web};

/// Where the usage service answers.
const ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";

/// The beta the usage service answers a login token under.
const BETA: [(&str, &str); 1] = [("anthropic-beta", "oauth-2025-04-20")];

/// The limits Claude Code's usage service reports, where Claude Code is
/// logged in to a plan.
pub(super) fn read() -> Option<Limits> {
    let login = login()?;
    let token = login["accessToken"]
        .as_str()
        .filter(|token| !token.is_empty())?;
    let usage = web::get(ENDPOINT, &format!("Bearer {token}"), &BETA)?;
    let windows = usage["limits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(window)
        .collect::<Vec<_>>();
    if windows.is_empty() {
        return None;
    }
    Limits::of(login["subscriptionType"].as_str(), windows)
}

/// The one window `limit` describes, where it is one of the plan's.
fn window(limit: &Value) -> Option<Window> {
    let label = match limit["kind"].as_str()? {
        "session" => "5-hour".to_owned(),
        "weekly_all" => "Weekly".to_owned(),
        "weekly_scoped" => format!(
            "Weekly {}",
            limit["scope"]["model"]["display_name"].as_str()?
        ),
        _ => return None,
    };
    Some(Window {
        label,
        used: limit["percent"].as_f64()?,
        resets: clock::moment(&limit["resets_at"]),
    })
}

/// The login Claude Code keeps: in the macOS keychain, or in the
/// credentials file in its configuration directory.
fn login() -> Option<Value> {
    let stored = keychain().or_else(|| fs::read_to_string(credentials()?).ok())?;
    let stored: Value = serde_json::from_str(&stored).ok()?;
    Some(stored["claudeAiOauth"].clone()).filter(Value::is_object)
}

/// Where Claude Code keeps its credentials file: in `CLAUDE_CONFIG_DIR`, or
/// in `.claude` in the home.
fn credentials() -> Option<PathBuf> {
    env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| Some(env::home_dir()?.join(".claude")))
        .map(|directory| directory.join(".credentials.json"))
}

/// The login Claude Code keeps in the macOS keychain, read from what
/// `security` writes out rather than handed to anything on a command line.
#[cfg(target_os = "macos")]
fn keychain() -> Option<String> {
    let found = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let stored = String::from_utf8(found.stdout).ok()?.trim().to_owned();
    (found.status.success() && !stored.is_empty()).then_some(stored)
}

/// The login Claude Code keeps in a keychain, which it does only on macOS.
#[cfg(not(target_os = "macos"))]
fn keychain() -> Option<String> {
    None
}
