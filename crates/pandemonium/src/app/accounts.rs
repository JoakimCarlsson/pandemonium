//! Account selection at the existing agent-launch seam.

use pm_acp::Agent;
use pm_core::Scope;

use super::App;
use crate::picker::{Choice, Kind, Row};

impl App {
    /// Offers accounts for the originating conversation's agent and worktree.
    pub(super) fn show_agent_accounts(&mut self, session: crate::agent::TalkId) {
        let Some(talk) = self.agents.get(session) else {
            return;
        };
        self.open_picker(Kind::Accounts(Some(talk.scope()), talk.agent()));
    }

    /// Offers separate accounts, with creation and removal in the same picker.
    pub(super) fn account_rows(
        &self,
        scope: Option<Scope>,
        agent: Agent,
        removing: bool,
    ) -> Vec<Row> {
        let mut rows = Vec::new();
        if !removing && scope.is_some() {
            rows.push(Row {
                section: None,
                label: "Default account".to_owned(),
                detail: "Use the agent's existing login".to_owned(),
                choice: Choice::Account(scope, agent, None),
                enabled: true,
            });
        }
        if !removing {
            rows.push(Row {
                section: None,
                label: "Sign in or switch default account…".to_owned(),
                detail: "Use the provider's sign-in flow for its shared default account".to_owned(),
                choice: Choice::AccountLogin(scope, agent, None),
                enabled: scope.is_some(),
            });
        }
        rows.extend(self.accounts.profiles(agent).map(|profile| Row {
            section: Some(agent.name),
            label: profile.name().to_owned(),
            detail: match (removing, profile.organisation()) {
                (true, _) => "Remove profile; retain login storage".to_owned(),
                (false, Some(id)) => format!(
                        "{} · {id}",
                        self.accounts
                            .organisation_kind(agent)
                            .unwrap_or("Organisation")
                    ),
                (false, None) => "Separate account".to_owned(),
            },
            choice: if removing {
                Choice::RemoveAccount(scope, agent, profile.id().to_owned())
            } else {
                Choice::Account(scope, agent, Some(profile.id().to_owned()))
            },
            enabled: removing || scope.is_some(),
        }));
        if !removing && crate::config::Accounts::supports(agent) {
            rows.extend([
                Row {
                    section: None,
                    label: "Create account profile…".to_owned(),
                    detail: match scope {
                        Some(_) => "Work, personal or organisation; sign in with the agent",
                        None => "Open a project to add and sign in to an account",
                    }
                    .to_owned(),
                    choice: Choice::NewAccount(scope, agent),
                    enabled: scope.is_some(),
                },
                Row {
                    section: None,
                    label: "Sign in to account profile…".to_owned(),
                    detail: "Choose an account or organisation through the provider".to_owned(),
                    choice: Choice::AccountLogins(scope, agent),
                    enabled: scope.is_some() && self.accounts.profiles(agent).next().is_some(),
                },
                Row {
                    section: None,
                    label: "Remove account profile…".to_owned(),
                    detail: String::new(),
                    choice: Choice::AccountRemoval(scope, agent),
                    enabled: self.accounts.profiles(agent).next().is_some(),
                },
            ]);
        }
        rows
    }

    /// Offers existing profiles for an explicit provider sign-in.
    pub(super) fn account_login_rows(&self, scope: Option<Scope>, agent: Agent) -> Vec<Row> {
        self.account_rows(scope, agent, false)
            .into_iter()
            .filter_map(|mut row| {
                let Choice::Account(scope, agent, Some(id)) = row.choice else {
                    return None;
                };
                row.choice = Choice::AccountLogin(scope, agent, Some(id));
                row.detail = "Sign in and choose your account or organisation".to_owned();
                Some(row)
            })
            .collect()
    }

    /// Starts provider sign-in for a chosen existing profile in its captured worktree.
    pub(super) fn authenticate_account(
        &mut self,
        scope: Option<Scope>,
        agent: Agent,
        id: Option<&str>,
    ) {
        self.open_account(scope, agent, id, true);
    }

    /// Creates a separate account and starts the provider's login in its worktree.
    pub(super) fn create_account(&mut self, scope: Option<Scope>, agent: Agent, name: &str) {
        let Some(scope) = scope.or_else(|| self.scope()) else {
            self.notices
                .trouble("Open a project to add and sign in to an account", None);
            self.open_picker(Kind::Accounts(None, agent));
            return;
        };
        let scope = Some(scope);
        if !self.accounts.valid_name(agent, name) {
            self.notices
                .trouble("Use a unique profile name of 1–80 characters", None);
            self.open_picker_with(Kind::NewAccount(scope, agent), Vec::new(), name.to_owned());
            return;
        }
        let Some(profile) = self.accounts.create(agent, name) else {
            self.notices.trouble(
                "Could not create profile: editor storage must be writable",
                None,
            );
            self.open_picker_with(Kind::NewAccount(scope, agent), Vec::new(), name.to_owned());
            return;
        };
        self.store();
        self.open_account(scope, agent, Some(profile.id()), true);
    }

    /// Starts an account's sole login method once, leaving multiple methods for selection.
    pub(super) fn start_account_logins(&mut self) {
        for session in std::mem::take(&mut self.account_logins) {
            let Some(talk) = self.agents.get(session) else {
                continue;
            };
            if talk.is_ready() || !talk.is_running() {
                continue;
            }
            match talk.logins().len() {
                0 => {
                    self.account_logins.insert(session);
                }
                1 => self.log_in_agent(session, 0),
                _ => {}
            }
        }
    }

    /// Removes metadata through config without modifying a running agent's files.
    pub(super) fn remove_account(&mut self, scope: Option<Scope>, agent: Agent, id: &str) {
        self.accounts.remove(agent, id);
        self.store();
        self.open_picker(Kind::Accounts(scope, agent));
    }

    /// Starts the chosen account in the worktree captured when selection began.
    pub(super) fn start_account(&mut self, scope: Option<Scope>, agent: Agent, id: Option<&str>) {
        self.open_account(scope, agent, id, false);
    }

    /// Resolves an account once and opens it with the requested authentication behavior.
    fn open_account(&mut self, scope: Option<Scope>, agent: Agent, id: Option<&str>, login: bool) {
        let Some(scope) = scope else {
            return;
        };
        let profile = match id {
            Some(id) => match self.accounts.find(agent, id).cloned() {
                Some(profile) => Some(profile),
                None => return,
            },
            None => None,
        };
        let Some(root) = self.root_of(scope) else {
            return;
        };
        self.open_agent(
            scope.project(),
            scope.session(),
            &root,
            agent,
            profile.as_ref(),
            login,
        );
    }
}
