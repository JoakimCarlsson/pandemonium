//! Keeping the logins of one agent apart, by giving each its own home folder.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::agent::Agent;

/// The home folder in use for each agent, by the agent's id; an agent that has none uses its own default.
static ACTIVE: Mutex<Option<HashMap<&'static str, PathBuf>>> = Mutex::new(None);

/// The variable that moves an agent's config and login to another folder, when it has one.
fn variable(agent: Agent) -> Option<&'static str> {
    match agent.id {
        "claude-code" => Some("CLAUDE_CONFIG_DIR"),
        "codex" => Some("CODEX_HOME"),
        "grok" => Some("GROK_HOME"),
        _ => None,
    }
}

impl Agent {
    /// Whether the agent can keep more than one login.
    pub fn has_accounts(self) -> bool {
        variable(self).is_some()
    }

    /// Makes every agent started from now on use `home` for its config and login, or its default one when there is none.
    pub fn use_home(self, home: Option<PathBuf>) {
        let mut active = ACTIVE.lock().unwrap_or_else(|held| held.into_inner());
        let held = active.get_or_insert_with(HashMap::new);
        match home {
            Some(home) => {
                share_setup(self, &home);
                held.insert(self.id, home);
            }
            None => {
                held.remove(self.id);
            }
        }
    }

    /// The home folder in use for the agent, when it is not its default.
    pub(crate) fn home(self) -> Option<PathBuf> {
        ACTIVE.lock().ok()?.as_ref()?.get(self.id).cloned()
    }

    /// The environment to start the agent with: its home variable pointing at the folder in use.
    pub(crate) fn environment(self) -> Option<(&'static str, PathBuf)> {
        Some((variable(self)?, self.home()?))
    }

    /// Signs the agent in under the home folder in use, by the first way it offers, and returns once that is over.
    pub fn sign_in(self) -> Result<(), String> {
        if !self.startable() {
            return Err("the agent is not installed".to_owned());
        }
        let mut client = crate::client::Client::start(self.command(&[]))?;
        let shaken = client.initialize(std::time::Duration::from_secs(180))?;
        drop(client);
        let logins = crate::client::logins(&shaken["authMethods"]);
        let login = logins
            .first()
            .ok_or("the agent offers no way to sign in")?;
        self.log_in(login)
    }
}

/// What of an agent's own folder an account folder shares, so that a new account has the skills and plugins the reader already has.
fn shared(agent: Agent) -> &'static [&'static str] {
    match agent.id {
        "claude-code" => &[
            "CLAUDE.md",
            "AGENTS.md",
            "rules",
            "skills",
            "commands",
            "agents",
            "hooks",
            "output-styles",
            "workflows",
            "plugins/cache",
            "plugins/marketplaces",
        ],
        "codex" => &[
            "AGENTS.md",
            "AGENTS.override.md",
            "skills",
            "rules",
            "agents",
            "plugins/cache",
            "plugins/marketplaces",
        ],
        "grok" => &[
            "AGENTS.md",
            "GROK.md",
            "skills",
            "commands",
            "agents",
            "rules",
            "hooks",
            "plugins",
            "installed-plugins",
        ],
        _ => &[],
    }
}

/// Links what an agent's own folder shares into the account folder `home`, leaving what is already there.
fn share_setup(agent: Agent, home: &std::path::Path) {
    let Some(source) = std::env::home_dir().map(|own| {
        match agent.id {
            "codex" => std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| own.join(".codex")),
            "grok" => std::env::var_os("GROK_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| own.join(".grok")),
            _ => own.join(".claude"),
        }
    }) else {
        return;
    };
    for entry in shared(agent) {
        let from = source.join(entry);
        let to = home.join(entry);
        if !from.exists() || to.symlink_metadata().is_ok() {
            continue;
        }
        if let Some(parent) = to.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(&from, &to);
    }
}
