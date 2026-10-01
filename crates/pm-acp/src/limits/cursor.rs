//! Cursor's limits, as its dashboard service reports them.
//!
//! The agent says nothing of them, but the service the dashboard is drawn
//! from answers with the token the Cursor CLI keeps once it is logged in.
//! The service is undocumented and may change without notice.

use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use super::{Limits, Window, clock, web};

/// Where the dashboard service answers.
const SERVICE: &str = "https://api2.cursor.sh/aiserver.v1.DashboardService";

/// The share of the period's included usage each kind of model has taken,
/// by the field it is reported under, with what a header calls it.
const SHARES: [(&str, &str); 2] = [("autoPercentUsed", "Auto"), ("apiPercentUsed", "API")];

/// The limits Cursor's dashboard reports, where the CLI is logged in.
pub(super) fn read() -> Option<Limits> {
    let authorization = format!("Bearer {}", token()?);
    let usage = web::post(&format!("{SERVICE}/GetCurrentPeriodUsage"), &authorization)?;
    let plan = web::post(&format!("{SERVICE}/GetPlanInfo"), &authorization);
    let resets = clock::moment(&usage["billingCycleEnd"]);
    let shares = &usage["planUsage"];
    let mut windows = SHARES
        .iter()
        .filter_map(|(field, label)| {
            Some(Window {
                label: (*label).to_owned(),
                used: shares[*field].as_f64()?,
                resets,
            })
        })
        .collect::<Vec<_>>();
    if windows.is_empty() {
        windows.extend(included(shares).map(|used| Window {
            label: "Included".to_owned(),
            used,
            resets,
        }));
    }
    let plan = plan
        .as_ref()
        .and_then(|plan| plan["planInfo"]["planName"].as_str());
    Limits::of(plan, windows)
}

/// The share of the included spend used, where a limit to it is reported.
fn included(shares: &Value) -> Option<f64> {
    let limit = shares["limit"].as_f64().filter(|limit| *limit > 0.0)?;
    Some(shares["includedSpend"].as_f64()? / limit * 100.0)
}

/// The access token the Cursor CLI keeps, where it is logged in.
fn token() -> Option<String> {
    keychain().or_else(|| {
        stored_files().into_iter().find_map(|file| {
            let stored: Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
            stored["accessToken"]
                .as_str()
                .filter(|token| !token.is_empty())
                .map(str::to_owned)
        })
    })
}

/// The access token the Cursor CLI keeps in the macOS keychain, read from
/// what `security` writes out rather than handed to anything on a command
/// line.
#[cfg(target_os = "macos")]
fn keychain() -> Option<String> {
    let found = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "cursor-access-token", "-w"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let token = String::from_utf8(found.stdout).ok()?.trim().to_owned();
    (found.status.success() && !token.is_empty()).then_some(token)
}

/// The access token the Cursor CLI keeps in a keychain, which it does only
/// on macOS.
#[cfg(not(target_os = "macos"))]
fn keychain() -> Option<String> {
    None
}

/// The files the Cursor CLI may keep its login in on this platform, most
/// likely first.
fn stored_files() -> Vec<PathBuf> {
    let home = env::home_dir();
    let config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(home.as_ref()?.join(".config")));
    let app_data = env::var_os("APPDATA").map(PathBuf::from);
    [
        config.map(|config| config.join("cursor").join("auth.json")),
        app_data.map(|app_data| app_data.join("Cursor").join("auth.json")),
        home.map(|home| home.join(".cursor").join("auth.json")),
    ]
    .into_iter()
    .flatten()
    .collect()
}
