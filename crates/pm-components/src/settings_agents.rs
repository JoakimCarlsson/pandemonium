//! The page of the agents and their accounts: a card for each agent with how it signs in, its switch, and its accounts or its API key.

use crate::areas::SettingsEvent;
use crate::button;
use crate::target::Target;
use crate::theme;
use pm_harness::SignIn;
use rustle::Modifier;
use rustle::TextFieldState;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{Icon, phosphor};

use crate::theme::{Radius, Role};

use crate::Badge;
use crate::button::Variant;
use crate::input::Input;
use crate::parts::glyph;
use crate::settings_row::switch;
use crate::settings_rows::{
    ACCOUNT_ADDS, ACCOUNT_COLUMNS, ACCOUNT_CONTROLS, ACCOUNT_REMOVES, AGENT_SWITCHES, DEFAULT_USES,
    KEY_REMOVES, KEY_SETS, SIGN_INS,
};
use crate::settings_servers::{boxed, flat_row, form_buttons, name_tone, note, paragraph, titled};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};

/// What one of the buttons of an added account does, by its column in [`ACCOUNT_CONTROLS`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountControl {
    /// Makes the account the one new sessions of its agent use.
    Use,
    /// Signs the account in, or in again.
    SignIn,
    /// Opens the box for the account's API key.
    SetKey,
    /// Forgets the account's saved API key.
    RemoveKey,
    /// Opens the box for the account's name.
    Rename,
}

impl AccountControl {
    /// Every button an added account has, in the order of their columns.
    const ALL: [Self; ACCOUNT_COLUMNS] = [
        Self::Use,
        Self::SignIn,
        Self::SetKey,
        Self::RemoveKey,
        Self::Rename,
    ];

    /// The name of this button of the login at `login` in the list of logins, or nothing when the page has no room for that many.
    pub fn id(self, login: usize) -> Option<Target> {
        let column = Self::ALL.iter().position(|held| *held == self)?;
        ACCOUNT_CONTROLS.cell(login, column, ACCOUNT_COLUMNS)
    }

    /// The login, by its place in the list of logins, and the button that `id` names, when it names one.
    fn of(id: Target) -> Option<(usize, Self)> {
        let (login, column) = ACCOUNT_CONTROLS.cell_of(id, ACCOUNT_COLUMNS)?;
        Some((login, *Self::ALL.get(column)?))
    }
}

/// The button opening the box for the API key of the agent at `agent`, or nothing when the page has no room for that many agents.
pub fn key_set_id(agent: usize) -> Option<Target> {
    KEY_SETS.id(agent)
}

/// How far an agent's rows stand in, so that they line up with its name, in spacing steps.
const ROW_INSET: u32 = 11;

/// How far the row adding an account stands in, so that its button's glyph lines up with the names above, in spacing steps.
const ADD_INSET: u32 = 8;

/// The box for an agent's API key while it is open.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyForm {
    /// The agent, by position.
    agent: usize,
    /// The added account the key is for, by id, or nothing for the agent's default account.
    login: Option<String>,
    /// The key being typed, shown as dots.
    key: TextFieldState,
}

/// The box for an added account's name while it is open.
#[derive(Clone, Debug, PartialEq)]
pub struct NameForm {
    /// The account, by its place in the list of logins.
    login: usize,
    /// The name being typed.
    name: TextFieldState,
}

impl WorkspaceState {
    /// Carries out `action` if it presses a part of the agents page, and says whether it did.
    pub(crate) fn agents_pressed(&mut self, action: Target) -> bool {
        if action == Target::Settings(SettingsEvent::KeySave) {
            self.save_key();
        } else if action == Target::Settings(SettingsEvent::KeyCancel) {
            self.settings_servers.key_form = None;
        } else if action == Target::Settings(SettingsEvent::NameSave) {
            self.save_name();
        } else if action == Target::Settings(SettingsEvent::NameCancel) {
            self.settings_servers.name_form = None;
        } else if let Some(agent) = KEY_SETS.index_of(action) {
            self.open_key_form(agent, None);
        } else if let Some(agent) = KEY_REMOVES.index_of(action) {
            self.send_key(agent, None, None);
        } else if let Some(agent) = SIGN_INS.index_of(action) {
            self.outbox.push(Outgoing::SignIn { agent, login: None });
        } else if let Some(agent) = DEFAULT_USES.index_of(action) {
            self.use_login(agent, None);
        } else if let Some(agent) = AGENT_SWITCHES.index_of(action) {
            self.toggle_agent_shown(agent);
        } else if let Some(agent) = ACCOUNT_ADDS.index_of(action) {
            self.add_account_from_settings(agent);
        } else if let Some(login) = ACCOUNT_REMOVES.index_of(action) {
            self.remove_account(login);
            self.settings_servers.name_form = None;
        } else if let Some((login, control)) = AccountControl::of(action) {
            self.account_pressed(login, control);
        } else {
            return false;
        }
        true
    }

    /// Carries out the button `control` of the added account at `position` in the list of logins.
    fn account_pressed(&mut self, position: usize, control: AccountControl) {
        let Some((owner, login, name)) = self.logins.get(position).cloned() else {
            return;
        };
        let Some(agent) = self.agent_position(&owner) else {
            return;
        };
        match control {
            AccountControl::Use => self.use_login(agent, Some(login)),
            AccountControl::SignIn => {
                self.settings_servers.login_identities.remove(&login);
                self.outbox.push(Outgoing::SignIn {
                    agent,
                    login: Some(login),
                });
            }
            AccountControl::SetKey => self.open_key_form(agent, Some(login)),
            AccountControl::RemoveKey => self.send_key(agent, None, Some(login)),
            AccountControl::Rename => {
                self.settings_servers.key_form = None;
                self.settings_servers.name_form = Some(NameForm {
                    login: position,
                    name: TextFieldState::new(&name),
                });
            }
        }
    }

    /// Adds an account to the agent at `agent`: one that signs in through the provider is signed in at once, and one that only takes an API key opens the box for its key.
    fn add_account_from_settings(&mut self, agent: usize) {
        let Some(owner) = self.agent_id_at(agent) else {
            return;
        };
        if pm_harness::agent(&owner).is_some_and(|held| held.sign_in.has_accounts()) {
            self.add_account(agent);
            return;
        }
        let id = self.new_login(owner);
        self.send_accounts();
        self.settings_servers
            .login_identities
            .insert(id.clone(), Some(None));
        self.open_key_form(agent, Some(id));
    }

    /// Makes the login `login` of the agent at `agent`, or its default account when there is none, the one new sessions of the agent use.
    pub(crate) fn use_login(&mut self, agent: usize, login: Option<String>) {
        let Some(owner) = self.agent_id_at(agent) else {
            return;
        };
        self.in_use.retain(|(by, _)| *by != owner);
        if let Some(id) = login {
            self.in_use.push((owner, id));
        }
        if let Some(identity) = self.identities.get_mut(agent) {
            *identity = None;
        }
        self.send_accounts();
    }

    /// Opens the box for the API key of the agent at `agent`, for its added account `login` or its default account when there is none.
    fn open_key_form(&mut self, agent: usize, login: Option<String>) {
        self.settings_servers.name_form = None;
        self.settings_servers.key_form = Some(KeyForm {
            agent,
            login,
            key: TextFieldState::new(""),
        });
    }

    /// Hands the server the API key typed in the open box, and puts the box away; a blank box is put away only.
    fn save_key(&mut self) {
        let Some(form) = self.settings_servers.key_form.take() else {
            return;
        };
        let key = form.key.text().trim().to_owned();
        if !key.is_empty() {
            self.send_key(form.agent, Some(key), form.login);
        }
    }

    /// Has the server keep `key` as the API key of the agent at `agent` for its added account `login`, or its default account when there is none, or forget the saved one, and forgets what the page said about that account until the server answers.
    fn send_key(&mut self, agent: usize, key: Option<String>, login: Option<String>) {
        if let Some(login) = &login {
            self.settings_servers.login_identities.remove(login);
        } else if let Some(held) = self.defaults.get_mut(agent) {
            *held = None;
        }
        self.outbox.push(Outgoing::SaveKey { agent, key, login });
    }

    /// Keeps the name typed for the account whose box is open, and puts the box away; a blank name is put away only.
    fn save_name(&mut self) {
        let Some(form) = self.settings_servers.name_form.take() else {
            return;
        };
        let name = form.name.text().trim().to_owned();
        let Some(held) = self.logins.get_mut(form.login).filter(|_| !name.is_empty()) else {
            return;
        };
        held.2 = name;
        self.send_accounts();
    }

    /// Asks who each added account is signed in as, for those not asked about yet.
    pub(crate) fn ask_login_identities(&mut self) {
        let unknown: Vec<(usize, String)> = self
            .logins
            .iter()
            .filter(|(_, id, _)| !self.settings_servers.login_identities.contains_key(id))
            .filter_map(|(owner, id, _)| Some((self.agent_position(owner)?, id.clone())))
            .collect();
        for (agent, login) in unknown {
            self.settings_servers
                .login_identities
                .insert(login.clone(), None);
            self.outbox.push(Outgoing::Identity {
                agent,
                naming: Some(login),
            });
        }
    }

    /// The box `id`, the one for an API key or the one for an account's name, while it is open.
    pub(crate) fn key_field(&self, id: Target) -> Option<&TextFieldState> {
        let servers = &self.settings_servers;
        if id == Target::Settings(SettingsEvent::NameField) {
            servers.name_form.as_ref().map(|form| &form.name)
        } else {
            servers.key_form.as_ref().map(|form| &form.key)
        }
    }
}

/// How an agent that signs in as `sign_in` does, in the words under its name; `keyed` says it also takes an API key, and `credentials` names the cloud credentials it reads instead of signing in here, such as Google Cloud's.
fn how_it_signs_in(sign_in: Option<SignIn>, keyed: bool, credentials: Option<&str>) -> String {
    match (sign_in, credentials) {
        (Some(SignIn::ApiKey), Some(credentials)) => format!("Uses an API key, or {credentials}"),
        (Some(SignIn::None), Some(credentials)) => format!("Uses {credentials}"),
        (Some(SignIn::Browser), _) if keyed => {
            "Signs in through your browser, or uses an API key".to_owned()
        }
        (Some(SignIn::DeviceCode), _) if keyed => {
            "Signs in with a one-time code, or uses an API key".to_owned()
        }
        (Some(SignIn::Browser), _) => "Signs in through your browser".to_owned(),
        (Some(SignIn::DeviceCode), _) => "Signs in with a one-time code".to_owned(),
        (Some(SignIn::ApiKey), None) => "Uses an API key".to_owned(),
        (Some(SignIn::None), None) => "Runs on a local server, no sign-in needed".to_owned(),
        (None, _) => String::new(),
    }
}

/// Emits the square an agent's glyph sits in at the start of its card, the glyph quieter when `dim`.
fn emblem(icon: Icon, dim: bool) {
    row_arranged(
        Modifier::new()
            .size(theme::sp(8), theme::sp(8))
            .background_rounded(theme::color(Role::SurfaceActive), Radius::Md.px()),
        Arrangement::Center,
        Alignment::Center,
        || glyph(icon, 4.5, if dim { Role::Subtle } else { Role::Text }),
    );
}

/// Emits the top of an agent's card: its glyph, its name and how it signs in, `keyed` when it also takes an API key and naming the cloud `credentials` it reads instead when it does, and the switch that offers it for new sessions.
fn header(
    state: &WorkspaceState,
    index: usize,
    name: &str,
    sign_in: Option<SignIn>,
    keyed: bool,
    credentials: Option<&str>,
) {
    let shown = state.agent_shown(index);
    let look = state.agent_look(index);
    let under = how_it_signs_in(sign_in, keyed, credentials);
    flat_row(3, || {
        rustle::row_arranged(
            Modifier::new().weight(1.0),
            Arrangement::SpacedBy(theme::sp(3)),
            Alignment::Center,
            || {
                emblem(look.icon, !shown);
                titled(
                    name,
                    name_tone(shown),
                    || {
                        if !shown {
                            Badge::new("Hidden").tone(Role::Subtle).build();
                        }
                    },
                    || {
                        if !under.is_empty() {
                            note(&under);
                        }
                    },
                );
            },
        );
        switch(shown, AGENT_SWITCHES.id(index));
    });
}

/// How an agent signs in and what it takes, as its accounts' rows need it.
#[derive(Clone, Copy)]
struct Signing {
    /// Whether an account signs in through the provider, in a browser or with a code.
    signs_in: bool,
    /// Whether an account can be given an API key.
    keyed: bool,
}

/// What a row says about who an account is: whom it is signed in as, its key, or that it is not signed in or still being asked; and whether it is signed in at all.
fn status_of(said: Option<&Option<String>>, signing: Signing) -> (String, bool) {
    match said {
        None => ("Checking…".to_owned(), false),
        Some(Some(who)) if who.starts_with("API key") || who == pm_harness::UNNAMED => {
            (who.clone(), true)
        }
        Some(Some(who)) => (format!("Signed in as {who}"), true),
        Some(None) if signing.signs_in => ("Not signed in".to_owned(), false),
        Some(None) => ("No API key".to_owned(), false),
    }
}

/// Emits the row of an account called `title`: whom it is signed in as, from `said`, under its name, marked when it is `in_use`, then the buttons every account has, by the names `ids` gives them (use it when it is not in use, sign it in, and set or forget its key), and what `more` emits after them.
#[track_caller]
fn account_row(
    title: &str,
    said: Option<&Option<String>>,
    signing: Signing,
    in_use: bool,
    ids: [Option<Target>; 4],
    more: impl FnOnce(),
) {
    let [use_id, sign_in, set_key, remove_key] = ids;
    let (status, signed_in) = status_of(said, signing);
    let keyed_now = status.starts_with("API key");
    flat_row(ROW_INSET, || {
        titled(
            title,
            name_tone(signed_in),
            || {
                if in_use {
                    Badge::new("In use").tone(Role::Subtle).build();
                }
            },
            || note(&status),
        );
        rustle::row_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            || {
                if signing.keyed {
                    if let Some(remove) = remove_key.filter(|_| status == pm_harness::SAVED_KEY) {
                        button::action("Remove key", remove, Variant::Ghost);
                    }
                    if let Some(set) = set_key {
                        let label = if keyed_now { "Replace key" } else { "Add key" };
                        button::action(label, set, Variant::Ghost);
                    }
                }
                if signing.signs_in
                    && let Some(sign_in) = sign_in
                {
                    let again = signed_in && !keyed_now;
                    let label = if again { "Sign in again" } else { "Sign in" };
                    button::action(label, sign_in, Variant::Ghost);
                }
                if let Some(use_id) = use_id.filter(|_| !in_use) {
                    button::action("Use", use_id, Variant::Outline);
                }
                more();
            },
        );
    });
}

/// Emits the box `id` an account's name or API key is typed into, holding `field` and showing `placeholder` while empty, with its buttons `save` and `cancel`, in place of the account's row; Enter in the box presses `save`.
#[track_caller]
fn form_row(field: &TextFieldState, id: Target, placeholder: &str, save: Target, cancel: Target) {
    let mut input = Input::new(field, id)
        .variant(crate::input::Variant::Bordered)
        .size(Size::Sm)
        .placeholder(placeholder)
        .submits(save)
        .modifier(Modifier::new().weight(1.0));
    if id == Target::Settings(SettingsEvent::KeyField) {
        input = input.leading_icon(phosphor::LOCK_SIMPLE).secure(true);
    }
    rustle::key(id, || {
        flat_row(ROW_INSET, || {
            input.build();
            form_buttons(Modifier::new(), cancel, save, true);
        });
    });
}

/// Emits the box an API key is typed into for the account `login` of the agent at `index`, or its default account when `login` is nothing, in place of the account's row; says whether that box is open.
#[track_caller]
fn key_row(state: &WorkspaceState, index: usize, login: Option<&str>) -> bool {
    let Some(form) = state
        .settings_servers
        .key_form
        .as_ref()
        .filter(|form| form.agent == index && form.login.as_deref() == login)
    else {
        return false;
    };
    form_row(
        &form.key,
        Target::Settings(SettingsEvent::KeyField),
        "Paste an API key",
        Target::Settings(SettingsEvent::KeySave),
        Target::Settings(SettingsEvent::KeyCancel),
    );
    true
}

/// Emits the rows of the accounts of the agent at `index`, whose id is `id`: its default account, the ones added, each with who it is signed in as and its buttons, and the button adding one.
fn account_rows(state: &WorkspaceState, index: usize, id: &str, signing: Signing) {
    if !key_row(state, index, None) {
        account_row(
            "Default account",
            state.defaults.get(index).and_then(Option::as_ref),
            signing,
            !state.in_use.iter().any(|(owner, _)| owner == id),
            [
                DEFAULT_USES.id(index),
                SIGN_INS.id(index),
                KEY_SETS.id(index),
                KEY_REMOVES.id(index),
            ],
            || {},
        );
    }
    for (position, (_, login, name)) in state
        .logins
        .iter()
        .enumerate()
        .filter(|(_, (owner, _, _))| owner == id)
    {
        if key_row(state, index, Some(login)) {
            continue;
        }
        if let Some(form) = state
            .settings_servers
            .name_form
            .as_ref()
            .filter(|form| form.login == position)
        {
            form_row(
                &form.name,
                Target::Settings(SettingsEvent::NameField),
                "Account name",
                Target::Settings(SettingsEvent::NameSave),
                Target::Settings(SettingsEvent::NameCancel),
            );
            continue;
        }
        let used = state
            .in_use
            .iter()
            .any(|(owner, held)| owner == id && held == login);
        let said = state
            .settings_servers
            .login_identities
            .get(login)
            .and_then(Option::as_ref);
        rustle::key(position, || {
            account_row(
                name,
                said,
                signing,
                used,
                [
                    AccountControl::Use.id(position),
                    AccountControl::SignIn.id(position),
                    AccountControl::SetKey.id(position),
                    AccountControl::RemoveKey.id(position),
                ],
                || {
                    if let Some(rename) = AccountControl::Rename.id(position) {
                        crate::queued::icon_button(phosphor::PENCIL_SIMPLE, "Rename", rename);
                    }
                    if let Some(remove) = ACCOUNT_REMOVES.id(position) {
                        crate::queued::icon_button(phosphor::TRASH, "Remove account", remove);
                    }
                },
            );
        });
    }
    flat_row(ADD_INSET, || {
        if let Some(add) = ACCOUNT_ADDS.id(index) {
            button::small_button("Add account", add, Variant::Ghost)
                .with_icon(phosphor::PLUS)
                .build();
        }
    });
}

/// Emits the card of the agent at `index`, `room` logical pixels across: its header, what the reader sets up for it when it needs something, then its accounts, each signed in or given a key of its own; nothing when there is no agent there.
fn agent_card(state: &WorkspaceState, index: usize, room: f32) {
    let Some(entry) = state.settings_servers.agents.get(index) else {
        return;
    };
    let agent = pm_harness::agent(&entry.id);
    let sign_in = agent.map(|agent| agent.sign_in);
    let keyed = agent.is_some_and(|agent| !agent.key_env.is_empty());
    let credentials = agent.and_then(|agent| agent.credentials);
    let hint = if entry.server {
        Some("A model server declared on the Model Servers page.".to_owned())
    } else {
        agent.and_then(|agent| agent.setup_hint())
    };
    let signing = Signing {
        signs_in: sign_in.is_some_and(SignIn::has_accounts),
        keyed,
    };
    boxed(|| {
        header(state, index, &entry.name, sign_in, keyed, credentials);
        if let Some(hint) = &hint {
            flat_row(ROW_INSET, || {
                crate::settings_servers::wrapped_note(
                    hint,
                    (room - ROW_INSET as f32 * 4.0 - 24.0).max(160.0),
                );
            });
        }
        if signing.signs_in || signing.keyed {
            account_rows(state, index, &entry.id, signing);
        }
    });
}

/// Emits the page of the agents, `room` logical pixels across: what `page` emits first, then what the page is for, then a card for each agent.
pub(crate) fn body(state: &WorkspaceState, page: impl FnOnce(), room: f32) {
    page();
    paragraph(
        "Every agent the harness offers. New sessions offer the ones signed in; switch one off to leave it out even then. Its sessions and accounts stay. An agent can keep several accounts, such as one per organisation; new sessions use the one marked in use. Skills, instructions, tools, memory and conversations are shared by every account and every agent.",
        room,
    );
    for index in 0..state.agent_slots() {
        rustle::key(index, || agent_card(state, index, room));
    }
}

/// Whether `id` names the box for an API key or the one for an account's name.
pub(crate) fn is_key_field(id: Target) -> bool {
    id == Target::Settings(SettingsEvent::KeyField)
        || id == Target::Settings(SettingsEvent::NameField)
}

#[cfg(test)]
/// Tests for the agents page.
mod tests {
    use super::*;
    use crate::areas::ComposerEvent;

    /// The position of the agent kept under `id`.
    fn position(state: &WorkspaceState, id: &str) -> usize {
        state.agent_position(id).unwrap()
    }

    /// Typing a key and saving it hands it to the server and forgets what the page said until it answers; a blank key is not sent, and removing sends nothing as the key.
    #[test]
    fn a_typed_key_is_sent_to_the_server() {
        let mut state = WorkspaceState::new(&[]);
        let openrouter = position(&state, "openrouter");
        assert!(state.agents_pressed(KEY_SETS.at(openrouter)));
        assert!(
            state
                .key_field(Target::Settings(SettingsEvent::KeyField))
                .is_some_and(|field| field.text().is_empty())
        );
        state = state.typed(Target::Settings(SettingsEvent::KeyField), " sk-or-1 ");
        assert!(state.agents_pressed(Target::Settings(SettingsEvent::KeySave)));
        assert!(
            state
                .key_field(Target::Settings(SettingsEvent::KeyField))
                .is_none()
        );
        let sent = state.take_outbox();
        assert!(sent.iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::SaveKey { agent, key: Some(key), .. } if *agent == openrouter && key == "sk-or-1"
        )));
        state.agents_pressed(KEY_SETS.at(openrouter));
        state.agents_pressed(Target::Settings(SettingsEvent::KeySave));
        assert!(state.take_outbox().is_empty());
        state.agents_pressed(KEY_REMOVES.at(openrouter));
        assert!(
            state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::SaveKey { key: None, .. }))
        );
        assert!(!state.agents_pressed(Target::Composer(ComposerEvent::Prompt)));
    }

    /// A cloud provider that reads the cloud's own credentials says so instead of passing for a local server, one that also takes a key names both, and a local server keeps its words.
    #[test]
    fn cloud_providers_name_their_credentials() {
        let label = |id: &str| {
            let agent = pm_harness::agent(id).unwrap();
            how_it_signs_in(
                Some(agent.sign_in),
                !agent.key_env.is_empty(),
                agent.credentials,
            )
        };
        assert_eq!(
            label("vertex"),
            "Uses Google Cloud credentials (gcloud or Application Default Credentials)"
        );
        assert!(
            label("bedrock").starts_with("Uses an API key, or AWS credentials"),
            "{}",
            label("bedrock")
        );
        assert_eq!(label("ollama"), "Runs on a local server, no sign-in needed");
        assert_eq!(label("openrouter"), "Uses an API key");
    }

    /// The default account can be signed in from its row.
    #[test]
    fn the_default_account_signs_in_from_its_row() {
        let mut state = WorkspaceState::new(&[]);
        let claude = position(&state, "claude-code");
        state.agents_pressed(SIGN_INS.at(claude));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::SignIn { agent, login: None } if *agent == claude
        )));
    }

    /// An added account is made the one in use, renamed, signed in and given a key of its own from its row, each reaching the server for that account alone.
    #[test]
    fn added_accounts_are_used_renamed_and_keyed_from_their_rows() {
        let mut state = WorkspaceState::new(&[]);
        let claude = position(&state, "claude-code");
        let owner = state.agent_id_at(claude).unwrap();
        state
            .logins
            .push((owner.clone(), "org2".to_owned(), "Account 2".to_owned()));
        state.take_outbox();
        assert!(state.agents_pressed(AccountControl::Use.id(0).unwrap()));
        assert_eq!(state.in_use, [(owner.clone(), "org2".to_owned())]);
        assert!(state.agents_pressed(DEFAULT_USES.id(claude).unwrap()));
        assert!(state.in_use.is_empty());
        assert!(state.agents_pressed(AccountControl::Rename.id(0).unwrap()));
        state = state.typed(Target::Settings(SettingsEvent::NameField), " Acme");
        assert!(state.agents_pressed(Target::Settings(SettingsEvent::NameSave)));
        assert_eq!(state.logins[0].2, "Account 2 Acme");
        state.take_outbox();
        assert!(state.agents_pressed(AccountControl::SignIn.id(0).unwrap()));
        assert!(state.agents_pressed(AccountControl::SetKey.id(0).unwrap()));
        state = state.typed(Target::Settings(SettingsEvent::KeyField), "sk-2");
        assert!(state.agents_pressed(Target::Settings(SettingsEvent::KeySave)));
        let sent = state.take_outbox();
        assert!(sent.iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::SignIn { agent, login: Some(login) } if *agent == claude && login == "org2"
        )));
        assert!(sent.iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::SaveKey { agent, key: Some(key), login: Some(login) }
                if *agent == claude && key == "sk-2" && login == "org2"
        )));
    }

    /// Adding an account to an agent that only takes an API key opens the box for that account's key instead of signing in.
    #[test]
    fn a_key_only_agent_adds_an_account_by_its_key() {
        let mut state = WorkspaceState::new(&[]);
        let openrouter = position(&state, "openrouter");
        state.take_outbox();
        assert!(state.agents_pressed(ACCOUNT_ADDS.id(openrouter).unwrap()));
        let sent = state.take_outbox();
        assert!(
            !sent
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::SignIn { .. }))
        );
        let login = state.logins.last().map(|(_, id, _)| id.clone()).unwrap();
        let form = state.settings_servers.key_form.clone().unwrap();
        assert_eq!(form.agent, openrouter);
        assert_eq!(form.login.as_deref(), Some(login.as_str()));
    }

    /// Who an added account is signed in as is asked once, and its answer is kept for its row; a key's name does not rename it.
    #[test]
    fn added_accounts_are_asked_who_they_are() {
        let mut state = WorkspaceState::new(&[]);
        let openrouter = position(&state, "openrouter");
        let owner = state.agent_id_at(openrouter).unwrap();
        state
            .logins
            .push((owner, "k2".to_owned(), "Account 2".to_owned()));
        state.take_outbox();
        state.ask_login_identities();
        state.ask_login_identities();
        let asked = state
            .take_outbox()
            .into_iter()
            .filter(|outgoing| matches!(outgoing, Outgoing::Identity { naming: Some(login), .. } if login == "k2"))
            .count();
        assert_eq!(asked, 1);
        let state = state.with_identity(
            openrouter,
            None,
            Some(pm_harness::SAVED_KEY.to_owned()),
            Some("k2".to_owned()),
        );
        assert_eq!(
            state.settings_servers.login_identities.get("k2"),
            Some(&Some(Some(pm_harness::SAVED_KEY.to_owned())))
        );
        assert_eq!(state.logins[0].2, "Account 2");
    }
}
