//! Keeping the logins of one agent apart, by giving each its own home folder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::agent::Agent;
use crate::product::Product;

/// The home folder in use for each agent, by the agent's id; an agent that has none uses its own default.
static ACTIVE: Mutex<Option<HashMap<&'static str, PathBuf>>> = Mutex::new(None);

impl Agent {
    /// Whether the agent can keep more than one login, which every agent can: one that has no variable for its login folder is run in a home folder of its own.
    pub fn has_accounts(self) -> bool {
        true
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

    /// The environment to start the agent with: its home variable pointing at the folder in use, or, when it has none, the home folder itself.
    pub(crate) fn environment(self) -> Option<(&'static str, PathBuf)> {
        Some((self.home.unwrap_or("HOME"), self.home()?))
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
        let login = logins.first().ok_or("the agent offers no way to sign in")?;
        self.log_in(login)
    }
}

/// What of an agent's own folder an account folder shares, so that a new account has the skills and plugins the reader already has.
fn shared(agent: Agent) -> &'static [&'static str] {
    match agent.product() {
        Some(Product::Claude) => &[
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
        Some(Product::Codex) => &[
            "AGENTS.md",
            "AGENTS.override.md",
            "skills",
            "rules",
            "agents",
            "plugins/cache",
            "plugins/marketplaces",
        ],
        Some(Product::Grok) => &[
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
        Some(Product::Copilot) => &[
            "agents",
            "skills",
            "instructions",
            "copilot-instructions.md",
            "installed-plugins",
        ],
        None => &[],
    }
}

/// Writes into an account folder the agent's settings file: the one the account has, with the reader's own from `source` laid over it and what the agent needs to sign in on its own and keep its login there over both.
fn seed(agent: Agent, home: &Path, source: Option<&Path>) {
    let (file, contents) = match agent.product() {
        Some(Product::Claude) => ("settings.json", r#"{"forceLoginMethod":"claudeai"}"#),
        Some(Product::Codex) => ("config.toml", "cli_auth_credentials_store = \"file\"\n"),
        _ => return,
    };
    let path = home.join(file);
    let account = std::fs::read_to_string(&path).ok();
    let user = source.and_then(|source| std::fs::read_to_string(source.join(file)).ok());
    let merged = if file.ends_with(".toml") {
        layered(
            account.as_deref(),
            user.as_deref(),
            contents,
            |text| text.parse::<toml::Table>().ok(),
            |table| toml::to_string(table).ok(),
        )
    } else {
        layered(
            account.as_deref(),
            user.as_deref(),
            contents,
            |text| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(text).ok(),
            |map| serde_json::to_string_pretty(map).ok(),
        )
    };
    let Some(merged) = merged else {
        return;
    };
    if account.as_deref() != Some(merged.as_str()) {
        let _ = std::fs::write(path, merged);
    }
}

/// The top-level keys of `account`, then `user` over them, then `seed` over both, written back out; `account` and `user` are each left out when they cannot be read.
fn layered<M, V>(
    account: Option<&str>,
    user: Option<&str>,
    seed: &str,
    parse: impl Fn(&str) -> Option<M>,
    write: impl Fn(&M) -> Option<String>,
) -> Option<String>
where
    M: Default + Extend<(String, V)> + IntoIterator<Item = (String, V)>,
{
    let mut layers = account.and_then(&parse).unwrap_or_default();
    layers.extend(user.and_then(&parse).unwrap_or_default());
    layers.extend(parse(seed)?);
    write(&layers)
}

/// The agent's own config folder, which an account folder shares its setup from; a home variable pointing at an account folder, as one inherited from an agent the app started, is passed over.
fn own_folder(agent: Agent) -> Option<PathBuf> {
    let product = agent.product()?;
    std::env::var_os(product.home_variable())
        .map(PathBuf::from)
        .filter(|folder| !is_account_folder(folder))
        .or_else(|| Some(std::env::home_dir()?.join(format!(".{}", product.slug()))))
}

/// Whether `folder` is an account folder, which the app keeps at `accounts/<agent>/<id>`.
fn is_account_folder(folder: &Path) -> bool {
    folder
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .is_some_and(|name| name == "accounts")
}

/// Links what an agent's own folder shares into the account folder `home`, leaving what is already there.
fn share_setup(agent: Agent, home: &Path) {
    let source = own_folder(agent);
    seed(agent, home, source.as_deref());
    if agent.home.is_none() {
        crate::overlay::build(agent, home);
        return;
    }
    let Some(source) = source else {
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
