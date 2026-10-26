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

    /// Signs the agent in under the home folder in use, by the first way it offers, and returns once that is over.
    pub fn sign_in(self) -> Result<(), String> {
        self.sign_in_in(self.home())
    }

    /// Signs the agent in under the account folder `home`, set up like any account folder, leaving the folder in use and the agent's default one alone.
    pub fn sign_in_folder(self, home: PathBuf) -> Result<(), String> {
        share_setup(self, &home);
        self.sign_in_in(Some(home))
    }

    /// Signs in using the offered methods and the session's explicit account folder.
    pub fn sign_in_in(self, home: Option<PathBuf>) -> Result<(), String> {
        if !self.startable() {
            return Err("the agent is not installed".to_owned());
        }
        let mut client = crate::client::Client::start(self.command_in(&[], home.clone()))?;
        let shaken = client.initialize(std::time::Duration::from_secs(180))?;
        drop(client);
        let logins = crate::client::logins(&shaken["authMethods"]);
        let login = logins.first().ok_or("the agent offers no way to sign in")?;
        self.log_in_in(login, home)
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

/// Writes into an account folder the agent's settings file: the one the account has, less the keys an earlier start took from the reader's own that it no longer has, with the reader's own from `source` laid over it and what the agent needs to sign in on its own and keep its login there over both.
fn seed(agent: Agent, home: &Path, source: Option<&Path>) {
    let (file, contents) = match agent.product() {
        Some(Product::Claude) => ("settings.json", r#"{"forceLoginMethod":"claudeai"}"#),
        Some(Product::Codex) => ("config.toml", "cli_auth_credentials_store = \"file\"\n"),
        _ => return,
    };
    let path = home.join(file);
    let record = home.join(format!("{file}.user-keys"));
    let account = std::fs::read_to_string(&path).ok();
    let user = source.and_then(|source| std::fs::read_to_string(source.join(file)).ok());
    let recorded = std::fs::read_to_string(&record).unwrap_or_default();
    let recorded: Vec<&str> = recorded.lines().collect();
    let merged = if file.ends_with(".toml") {
        layered(
            account.as_deref(),
            user.as_deref(),
            contents,
            &recorded,
            |text| text.parse::<toml::Table>().ok(),
            |table| toml::to_string(table).ok(),
        )
    } else {
        layered(
            account.as_deref(),
            user.as_deref(),
            contents,
            &recorded,
            |text| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(text).ok(),
            |map| serde_json::to_string_pretty(map).ok(),
        )
    };
    let Some((merged, user_keys)) = merged else {
        return;
    };
    if account.as_deref() != Some(merged.as_str()) && std::fs::write(&path, merged).is_err() {
        return;
    }
    if let Some(user_keys) = user_keys.filter(|keys| *keys != recorded) {
        let _ = std::fs::write(record, user_keys.join("\n"));
    }
}

/// The top-level keys of `account` less those in `recorded` that `user` no longer has, then `user` over them, then `seed` over both, written back out with the keys `user` has; `account` and `user` are each left out when they cannot be read, and nothing is dropped when `user` cannot be.
fn layered<M, V>(
    account: Option<&str>,
    user: Option<&str>,
    seed: &str,
    recorded: &[&str],
    parse: impl Fn(&str) -> Option<M>,
    write: impl Fn(&M) -> Option<String>,
) -> Option<(String, Option<Vec<String>>)>
where
    M: Default + Extend<(String, V)> + IntoIterator<Item = (String, V)> + FromIterator<(String, V)>,
{
    let user: Option<Vec<(String, V)>> =
        user.and_then(&parse).map(|user| user.into_iter().collect());
    let user_keys: Option<Vec<String>> = user
        .as_ref()
        .map(|user| user.iter().map(|(key, _)| key.clone()).collect());
    let removed = |key: &String| {
        user_keys
            .as_ref()
            .is_some_and(|keys| recorded.contains(&key.as_str()) && !keys.contains(key))
    };
    let mut layers: M = account
        .and_then(&parse)
        .unwrap_or_default()
        .into_iter()
        .filter(|(key, _)| !removed(key))
        .collect();
    layers.extend(user.unwrap_or_default());
    layers.extend(parse(seed)?);
    Some((write(&layers)?, user_keys))
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
pub(crate) fn share_setup(agent: Agent, home: &Path) {
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
