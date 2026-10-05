//! Inherited provider preferences, with profile overrides and authentication retained.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use serde_json::{Map, Value};

use super::account_setup::write;

/// Claude preferences that do not select or supply a provider identity.
const CLAUDE: &[&str] = &[
    "model",
    "effortLevel",
    "permissions",
    "hooks",
    "enabledPlugins",
    "extraKnownMarketplaces",
    "outputStyle",
    "statusLine",
    "language",
    "cleanupPeriodDays",
    "alwaysThinkingEnabled",
    "includeCoAuthoredBy",
    "respectGitignore",
    "plansDirectory",
    "attribution",
    "spinnerVerbs",
    "spinnerTipsEnabled",
    "terminalProgressBarEnabled",
    "prefersReducedMotion",
    "showTurnDuration",
    "autoUpdatesChannel",
    "skipDangerousModePermissionPrompt",
    "enableAllProjectMcpServers",
    "enabledMcpjsonServers",
    "disabledMcpjsonServers",
];

/// Codex preferences independent of login, provider selection and external state stores.
const CODEX: &[&str] = &[
    "model",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "model_verbosity",
    "plan_mode_reasoning_effort",
    "approval_policy",
    "sandbox_mode",
    "sandbox_workspace_write",
    "shell_environment_policy",
    "features",
    "agents",
    "skills",
    "plugins",
    "mcp_servers",
    "developer_instructions",
    "project_doc_max_bytes",
    "project_doc_fallback_filenames",
    "notify",
    "tui",
    "file_opener",
    "hide_agent_reasoning",
    "show_raw_agent_reasoning",
    "web_search",
    "personality",
    "feedback",
    "history",
    "check_for_update_on_startup",
];

/// Grok preferences independent of authentication, custom providers and managed policy.
const GROK: &[&str] = &[
    "cli",
    "agent",
    "models",
    "ui",
    "features",
    "skills",
    "compat",
    "plugins",
    "hooks",
    "permission",
    "sandbox",
    "shell_environment_policy",
    "mcp_servers",
    "toolset",
    "subagents",
    "lsp",
    "formatting",
];

/// Refreshes inherited settings without overwriting settings edited in the profile.
pub(super) fn refresh(
    source: &Path,
    destination: &Path,
    agent: &str,
    previous: &Value,
) -> io::Result<Value> {
    let name = match agent {
        "claude-code" => "settings.json",
        _ => "config.toml",
    };
    let shared = read(&source.join(name))?;
    let keys = match agent {
        "claude-code" => CLAUDE,
        "codex" => CODEX,
        _ => GROK,
    };
    let preset = if agent == "codex" {
        shared
            .get("profile")
            .and_then(Value::as_str)
            .and_then(|name| shared.get("profiles")?.get(name))
    } else {
        None
    };
    let inherited = Value::Object(
        [&shared]
            .into_iter()
            .chain(preset)
            .filter_map(Value::as_object)
            .flat_map(Map::iter)
            .filter(|(key, _)| keys.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), sanitize(value)))
            .collect(),
    );
    let path = destination.join(name);
    let current = read(&path)?;
    let mut merged = merge(
        Some(&current),
        previous.as_object().map(|_| previous),
        Some(&inherited),
    )
    .unwrap_or_else(|| Value::Object(Map::new()));
    if agent == "codex" {
        merged["cli_auth_credentials_store"] = Value::String("file".to_owned());
    }
    if merged != current {
        let text = if name.ends_with(".json") {
            serde_json::to_string_pretty(&merged).map_err(io::Error::other)?
        } else {
            toml::to_string_pretty(&merged).map_err(io::Error::other)?
        };
        write(&path, format!("{text}\n").as_bytes())?;
    }
    Ok(inherited)
}

/// Reads an object from JSON or TOML, distinguishing absence from invalid configuration.
fn read(path: &Path) -> io::Result<Value> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Value::Object(Map::new()));
        }
        Err(error) => return Err(error),
    };
    let value: Value = if path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        serde_json::from_str(&text).map_err(|_| {
            io::Error::other(format!("Invalid JSON preferences: {}", path.display()))
        })?
    } else {
        toml::from_str(&text).map_err(|_| {
            io::Error::other(format!("Invalid TOML preferences: {}", path.display()))
        })?
    };
    if !value.is_object() {
        return Err(io::Error::other("Provider preferences must be an object"));
    }
    Ok(value)
}

/// Removes credential-bearing fields and environment injection from inherited tables.
fn sanitize(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .filter(|(key, _)| shareable(key))
                .map(|(key, value)| (key.clone(), sanitize(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(sanitize).collect()),
        _ => value.clone(),
    }
}

/// Whether a nested preference can be inherited without importing authentication or state.
fn shareable(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace('-', "_");
    !matches!(
        key.as_str(),
        "env"
            | "set"
            | "http_headers"
            | "env_http_headers"
            | "headers"
            | "extra_headers"
            | "auth"
            | "authorization"
            | "credentials"
            | "password"
            | "secret"
            | "token"
            | "apikeyhelper"
            | "forceloginmethod"
            | "forceloginorguuid"
            | "forced_login_method"
            | "forced_chatgpt_workspace_id"
            | "cli_auth_credentials_store"
            | "experimental_bearer_token"
            | "bearer_token_env_var"
            | "env_key"
            | "storage_path"
            | "sqlite_home"
            | "session_path"
            | "install_dir"
    ) && !key.ends_with("@synced")
        && !key.ends_with("_token")
        && !key.ends_with("_api_key")
        && key != "api_key"
        && !key.ends_with("_secret")
        && !key.ends_with("_password")
}

/// Updates values still equal to the last inherited value and retains profile edits.
fn merge(
    current: Option<&Value>,
    previous: Option<&Value>,
    shared: Option<&Value>,
) -> Option<Value> {
    if current == previous || current.is_none() && previous.is_none() {
        return shared.cloned();
    }
    if current.is_some_and(Value::is_object)
        && previous.is_none_or(Value::is_object)
        && shared.is_none_or(Value::is_object)
    {
        let mut result = current
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let before = previous.and_then(Value::as_object);
        let after = shared.and_then(Value::as_object);
        let keys = before
            .into_iter()
            .chain(after)
            .flat_map(Map::keys)
            .collect::<BTreeSet<_>>();
        for key in keys {
            match merge(
                result.get(key),
                before.and_then(|map| map.get(key)),
                after.and_then(|map| map.get(key)),
            ) {
                Some(value) => {
                    result.insert(key.clone(), value);
                }
                None => {
                    result.remove(key);
                }
            }
        }
        return Some(Value::Object(result));
    }
    current.cloned()
}

/// Refreshes Claude's local marketplace records while leaving synced plugins to each account.
pub(super) fn plugins(source: &Path, destination: &Path, previous: &Value) -> io::Result<Value> {
    let mut inherited = Map::new();
    for name in ["known_marketplaces.json", "installed_plugins.json"] {
        let path = Path::new("plugins").join(name);
        let mut shared = read(&source.join(&path))?;
        if name == "installed_plugins.json"
            && let Some(plugins) = shared.get_mut("plugins").and_then(Value::as_object_mut)
        {
            plugins.retain(|name, installs| {
                if name.ends_with("@synced") {
                    return false;
                }
                if let Some(installs) = installs.as_array_mut() {
                    installs.retain(|install| {
                        install.get("scope").and_then(Value::as_str) == Some("user")
                    });
                    return !installs.is_empty();
                }
                false
            });
        }
        let shared = sanitize(&shared);
        let target = destination.join(&path);
        let current = read(&target)?;
        let merged = merge(Some(&current), previous.get(name), Some(&shared))
            .unwrap_or_else(|| Value::Object(Map::new()));
        if merged != current {
            fs::create_dir_all(
                target
                    .parent()
                    .ok_or_else(|| io::Error::other("Missing plugin parent"))?,
            )?;
            let text = serde_json::to_string_pretty(&merged).map_err(io::Error::other)?;
            write(&target, format!("{text}\n").as_bytes())?;
        }
        inherited.insert(name.to_owned(), shared);
    }
    Ok(Value::Object(inherited))
}
