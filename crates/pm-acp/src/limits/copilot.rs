//! Copilot's limits, as GitHub reports the premium-request quota.
//!
//! The agent says nothing of them, but GitHub answers
//! `copilot_internal/user` with the quota for a token of one of its own
//! OAuth apps, which is the one the `gh` login holds. The endpoint is
//! undocumented and may change without notice.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use super::{Limits, Window, clock, web};
use crate::agent;

/// Where GitHub answers with the signed-in user's Copilot quotas.
const ENDPOINT: &str = "https://api.github.com/copilot_internal/user";

/// The host the `gh` login is read for.
const HOST: &str = "github.com";

/// The premium-request quota GitHub reports, where `gh` is logged in and
/// the account has a quota to report.
pub(super) fn read() -> Option<Limits> {
    let user = web::get(ENDPOINT, &format!("token {}", token()?), &[])?;
    let quota = &user["quota_snapshots"]["premium_interactions"];
    let remaining = quota["percent_remaining"]
        .as_f64()
        .filter(|_| quota["unlimited"] != true)?;
    let window = Window {
        label: "Premium requests".to_owned(),
        used: (100.0 - remaining).clamp(0.0, 100.0),
        resets: clock::moment(&user["quota_reset_date_utc"])
            .or_else(|| clock::moment(&user["quota_reset_date"])),
    };
    Limits::of(user["copilot_plan"].as_str(), vec![window])
}

/// The token the `gh` login holds: written in its hosts file, or kept in the
/// system keyring, where `gh` hands it over on its output.
fn token() -> Option<String> {
    hosts_file().or_else(keyring)
}

/// The token `gh` wrote into its hosts file for [`HOST`], where it keeps it
/// there rather than in a keyring.
fn hosts_file() -> Option<String> {
    let hosts = fs::read_to_string(config()?.join("hosts.yml")).ok()?;
    hosts
        .lines()
        .skip_while(|line| line.trim_end() != format!("{HOST}:"))
        .skip(1)
        .take_while(|line| line.starts_with(char::is_whitespace))
        .find_map(|line| line.trim().strip_prefix("oauth_token:"))
        .map(|token| token.trim().trim_matches(['"', '\'']).to_owned())
        .filter(|token| !token.is_empty())
}

/// Where `gh` keeps its configuration on this platform.
fn config() -> Option<PathBuf> {
    if let Some(directory) = env::var_os("GH_CONFIG_DIR") {
        return Some(PathBuf::from(directory));
    }
    if cfg!(windows) {
        return Some(PathBuf::from(env::var_os("APPDATA")?).join("GitHub CLI"));
    }
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(env::home_dir()?.join(".config")))
        .map(|config| config.join("gh"))
}

/// The token `gh` keeps in the system keyring, as it writes it out.
fn keyring() -> Option<String> {
    let gh = agent::installed("gh")?;
    let told = Command::new(gh)
        .args(["auth", "token", "--hostname", HOST])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let token = String::from_utf8(told.stdout).ok()?.trim().to_owned();
    (told.status.success() && !token.is_empty()).then_some(token)
}
