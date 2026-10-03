//! Initial provider settings for a separate account's native sign-in flow.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

use pm_acp::{Agent, Source};

/// What this agent calls the identity selected by an organisation UUID.
pub(super) fn organisation_kind(agent: Agent) -> Option<&'static str> {
    if matches!(agent.source, Source::Command) {
        return None;
    }
    match agent.id {
        "claude-code" => Some("Claude organisation"),
        "codex" => Some("ChatGPT workspace"),
        "grok" => Some("Grok team"),
        _ => None,
    }
}

/// Prepares new provider storage without constraining the organisation chosen at login.
pub(super) fn initialize(directory: &Path, agent: Agent) -> io::Result<()> {
    match agent.id {
        "claude-code" => write_new(
            directory,
            "settings.json",
            r#"{"forceLoginMethod":"claudeai"}"#,
        ),
        "codex" => write_new(
            directory,
            "config.toml",
            r#"cli_auth_credentials_store = "file"
"#,
        ),
        _ => Ok(()),
    }
}

/// Writes a fresh provider settings file without reading or replacing agent state.
fn write_new(directory: &Path, name: &str, text: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(name))?;
    file.write_all(text.as_bytes())
}
