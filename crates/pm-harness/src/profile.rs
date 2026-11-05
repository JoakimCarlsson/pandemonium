//! Keeping the logins of one agent apart, by giving each its own folder to keep its credentials in.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::agent::Agent;

/// The login folder in use for each agent, by the agent's id; an agent that has none keeps its login in the harness's own folder.
static ACTIVE: Mutex<Option<HashMap<&'static str, PathBuf>>> = Mutex::new(None);

impl Agent {
    /// Whether the agent can keep more than one login, which an agent whose sign-in names an account can.
    pub fn has_accounts(self) -> bool {
        self.sign_in.has_accounts()
    }

    /// Makes every conversation started from now on keep its login in `home`, or in the harness's own folder when there is none.
    pub fn use_home(self, home: Option<PathBuf>) {
        let mut active = ACTIVE.lock().unwrap_or_else(|held| held.into_inner());
        let held = active.get_or_insert_with(HashMap::new);
        match home {
            Some(home) => {
                held.insert(self.id, home);
            }
            None => {
                held.remove(self.id);
            }
        }
    }

    /// The login folder in use for the agent, when it is not the harness's own.
    pub(crate) fn home(self) -> Option<PathBuf> {
        ACTIVE.lock().ok()?.as_ref()?.get(self.id).cloned()
    }

    /// Signs the agent in under the login folder in use and returns once that is over; `code` is told the page and code to show when the provider signs in with one.
    ///
    /// # Errors
    ///
    /// Fails with the reason when the harness cannot be started or the login fails.
    pub fn sign_in(self, code: &dyn Fn(&str, &str)) -> Result<(), String> {
        self.sign_in_in(self.home(), code)
    }

    /// Signs the agent in under the login folder `home`, leaving the folder in use alone; `code` is told the page and code to show when the provider signs in with one.
    ///
    /// # Errors
    ///
    /// Fails with the reason when the harness cannot be started or the login fails.
    pub fn sign_in_folder(self, home: PathBuf, code: &dyn Fn(&str, &str)) -> Result<(), String> {
        self.sign_in_in(Some(home), code)
    }
}
