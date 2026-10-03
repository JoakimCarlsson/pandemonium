//! Named agent accounts stored through the editor's config seam.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use pm_acp::{Agent, Source};
use serde::{Deserialize, Serialize};

use super::{account_identity, paths};

/// Account metadata persisted in the editor's settings, without credentials.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Accounts {
    /// Named accounts whose storage is owned exclusively by their agent.
    profiles: Vec<Profile>,
}

/// A named account; neither its name nor its identity contains credentials.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Profile {
    /// The immutable directory name, independent of the displayed account name.
    id: String,
    /// The agent whose account this is.
    agent: String,
    /// The account's label, such as Work or Personal.
    name: String,
    /// An organisation restriction retained from profiles created with a UUID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    organisation: Option<String>,
}

impl Accounts {
    /// Whether this agent supports a separate config and credential store.
    pub fn supports(agent: Agent) -> bool {
        variable(agent).is_some()
    }

    /// The provider identity name displayed for a legacy organisation restriction.
    pub fn organisation_kind(&self, agent: Agent) -> Option<&'static str> {
        Self::supports(agent)
            .then(|| account_identity::organisation_kind(agent))
            .flatten()
    }

    /// The accounts offered for this agent with separate storage.
    pub fn profiles(&self, agent: Agent) -> impl Iterator<Item = &Profile> {
        let supported = Self::supports(agent);
        self.profiles.iter().filter(move |profile| {
            supported && profile.agent == agent.id && profile.directory().is_some()
        })
    }

    /// Resolves an offered profile without accepting another agent's identity.
    pub fn find(&self, agent: Agent, id: &str) -> Option<&Profile> {
        self.profiles(agent).find(|profile| profile.id == id)
    }

    /// Whether a new account has a valid, unique label for this agent.
    pub fn valid_name(&self, agent: Agent, name: &str) -> bool {
        let name = name.trim();
        Self::supports(agent)
            && !name.is_empty()
            && name.chars().count() <= 80
            && !name.chars().any(char::is_control)
            && !self.profiles(agent).any(|profile| profile.name == name)
    }

    /// Creates empty storage for provider sign-in and adds its metadata to settings.
    pub fn create(&mut self, agent: Agent, name: &str) -> Option<Profile> {
        let name = name.trim();
        if !self.valid_name(agent, name) {
            return None;
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos();
        let mut count = 0;
        loop {
            let profile = Profile {
                id: format!("{stamp:x}-{count:x}"),
                agent: agent.id.to_owned(),
                name: name.to_owned(),
                organisation: None,
            };
            let directory = profile.directory()?;
            std::fs::create_dir_all(directory.parent()?).ok()?;
            match create_private_directory(&directory) {
                Ok(()) => {
                    if account_identity::initialize(&directory, agent).is_err() {
                        let _ = std::fs::remove_dir_all(&directory);
                        return None;
                    }
                    self.profiles.push(profile.clone());
                    return Some(profile);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => count += 1,
                Err(_) => return None,
            }
        }
    }

    /// Removes metadata while retaining agent-owned storage and running sessions.
    pub fn remove(&mut self, agent: Agent, id: &str) {
        if !Self::supports(agent) {
            return;
        }
        self.profiles
            .retain(|profile| profile.agent != agent.id || profile.id != id);
    }
}

impl Profile {
    /// The identity a picker uses to select this account.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The label drawn beside the running agent.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The actual provider identity selected for login, rather than its display label.
    pub fn organisation(&self) -> Option<&str> {
        self.organisation.as_deref()
    }

    /// The storage directory, constrained to the editor's accounts directory.
    fn directory(&self) -> Option<PathBuf> {
        if !safe_component(&self.id) || !safe_component(&self.agent) {
            return None;
        }
        Some(
            paths::home()?
                .join("accounts")
                .join(&self.agent)
                .join(&self.id),
        )
    }

    /// The agent's own home variable, passed only through its environment.
    pub fn environment(&self, agent: Agent) -> Option<(String, String)> {
        if self.agent != agent.id {
            return None;
        }
        Some((
            variable(agent)?.to_owned(),
            self.directory()?.canonicalize().ok()?.to_str()?.to_owned(),
        ))
    }
}

/// Whether a stored identifier is a single safe directory component.
fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// The home variable for agents with separate account storage.
fn variable(agent: Agent) -> Option<&'static str> {
    if matches!(agent.source, Source::Command) {
        return None;
    }
    match agent.id {
        "claude-code" => Some("CLAUDE_CONFIG_DIR"),
        "codex" => Some("CODEX_HOME"),
        "grok" => Some("GROK_HOME"),
        _ => None,
    }
}

/// Creates an empty account directory with owner-only permissions on Unix.
fn create_private_directory(directory: &std::path::Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(directory)
}
