//! Who an agent is signed in as, as the harness's own logins say.

use std::path::PathBuf;

use omc_wire::{KeySource, SignIn};

use crate::agent::Agent;

/// What an agent signed in with an API key the reader saved is called in place of an account.
pub const SAVED_KEY: &str = "API key saved";

/// Who `agent` is signed in as under the login folder in use for it: an organisation or an address.
pub fn account(agent: Agent) -> Option<String> {
    account_in(agent, agent.home())
}

/// Who `agent` is signed in as in the harness's own login folder.
pub fn default_account(agent: Agent) -> Option<String> {
    account_in(agent, None)
}

/// Who `agent` is signed in as in the login folder `home`.
pub fn account_in_folder(agent: Agent, home: PathBuf) -> Option<String> {
    account_in(agent, Some(home))
}

/// What a login that names no account is called, as for GitHub Copilot.
pub const UNNAMED: &str = "Signed in";

/// Who `agent` is signed in as in the login folder `home`, or in the harness's own when there is none; a login that names no account is [`UNNAMED`], and an agent signed in with an API key says where the key comes from. An agent that takes a key or an account login is reported by its key when it has one, and by its login otherwise.
fn account_in(agent: Agent, home: Option<PathBuf>) -> Option<String> {
    if !agent.key_env.is_empty() || agent.sign_in == SignIn::ApiKey {
        let key =
            omc_session::key_source(agent.provider, home.clone()).map(|source| match source {
                KeySource::Environment { name } => format!("API key from {name}"),
                KeySource::Saved => SAVED_KEY.to_owned(),
            });
        if key.is_some() || agent.sign_in == SignIn::ApiKey {
            return key;
        }
    }
    omc_session::account(agent.provider, home.clone())
        .or_else(|| omc_session::signed_in(agent.provider, home).then(|| UNNAMED.to_owned()))
}

impl Agent {
    /// Keeps `key` as the API key the agent is signed in with, in the harness's own login folder, or forgets the saved key when `key` is nothing or blank.
    ///
    /// # Errors
    ///
    /// Fails with the reason when the agent takes no API key or the logins cannot be written.
    pub fn save_key(self, key: Option<&str>) -> Result<(), String> {
        self.save_key_in(key, None)
    }

    /// Keeps `key` as the API key the agent is signed in with in the login folder `home`, or in the harness's own when there is none, or forgets the key saved there when `key` is nothing or blank.
    ///
    /// # Errors
    ///
    /// Fails with the reason when the agent takes no API key or the logins cannot be written.
    pub fn save_key_in(self, key: Option<&str>, home: Option<PathBuf>) -> Result<(), String> {
        omc_session::save_key(self.provider, key, home)
    }
}
