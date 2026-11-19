//! The page of the agents and their accounts: a card for each agent with how it signs in, its switch, and its accounts or its API key.

use crate::theme;
use crate::theme::sp;
use pm_harness::SignIn;
use rustle::Modifier;
use rustle::TextFieldState;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use crate::focus::FocusId;
use crate::icons::IconName;
use crate::select::styled_text;
use crate::theme::{Radius, Role};

use crate::button::Variant;
use crate::input::Input;
use crate::parts::{glyph, line};
use crate::settings_ids::{
    ACCOUNT_ADDS, ACCOUNT_COLUMNS, ACCOUNT_CONTROLS, ACCOUNT_REMOVES, AGENT_SWITCHES, DEFAULT_USES,
    KEY_REMOVES, KEY_SETS, SIGN_INS, SINGLES,
};
use crate::settings_row::switch;
use crate::settings_servers::{boxed, flat_row, paragraph};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Badge, Button};

/// The box an API key is typed into.
pub const KEY_FIELD: FocusId = SINGLES.at(90);

/// Keeps the API key typed.
pub const KEY_SAVE: FocusId = SINGLES.at(91);

/// Puts the box for an API key away.
pub const KEY_CANCEL: FocusId = SINGLES.at(92);

/// The box a new name for an added account is typed into.
pub const NAME_FIELD: FocusId = SINGLES.at(80);

/// Keeps the name typed for an added account.
pub const NAME_SAVE: FocusId = SINGLES.at(81);

/// Puts the box for an account's name away.
pub const NAME_CANCEL: FocusId = SINGLES.at(82);

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
    pub fn id(self, login: usize) -> Option<FocusId> {
        let column = Self::ALL.iter().position(|held| *held == self)?;
        ACCOUNT_CONTROLS.cell(login, column, ACCOUNT_COLUMNS)
    }

    /// The login, by its place in the list of logins, and the button that `id` names, when it names one.
    fn of(id: FocusId) -> Option<(usize, Self)> {
        let (login, column) = ACCOUNT_CONTROLS.cell_of(id, ACCOUNT_COLUMNS)?;
        Some((login, *Self::ALL.get(column)?))
    }
}

/// The button opening the box for the API key of the agent at `agent`, or nothing when the page has no room for that many agents.
pub fn key_set_id(agent: usize) -> Option<FocusId> {
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
    pub(crate) fn agents_pressed(&mut self, action: FocusId) -> bool {
        if action == KEY_SAVE {
            self.save_key();
        } else if action == KEY_CANCEL {
            self.settings_servers.key_form = None;
        } else if action == NAME_SAVE {
            self.save_name();
        } else if action == NAME_CANCEL {
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
        let signs_in = self
            .agent_id_at(agent)
            .and_then(|id| pm_harness::agent(&id))
            .is_some_and(|held| held.sign_in.has_accounts());
        if signs_in {
            self.add_account(agent);
            return;
        }
        let Some(owner) = self.agent_id_at(agent) else {
            return;
        };
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        let id = format!("{stamp:x}");
        let name = self.placeholder_name(&owner);
        self.logins.push((owner, id.clone(), name));
        self.outbox.push(Outgoing::Accounts {
            logins: self.logins.clone(),
            in_use: self.in_use.clone(),
        });
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
        self.outbox.push(Outgoing::Accounts {
            logins: self.logins.clone(),
            in_use: self.in_use.clone(),
        });
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
        match &login {
            Some(login) => {
                self.settings_servers.login_identities.remove(login);
            }
            None => {
                if let Some(held) = self.defaults.get_mut(agent) {
                    *held = None;
                }
            }
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
        self.outbox.push(Outgoing::Accounts {
            logins: self.logins.clone(),
            in_use: self.in_use.clone(),
        });
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
    pub(crate) fn key_field(&self, id: FocusId) -> Option<&TextFieldState> {
        if id == NAME_FIELD {
            return self
                .settings_servers
                .name_form
                .as_ref()
                .map(|form| &form.name);
        }
        self.settings_servers
            .key_form
            .as_ref()
            .map(|form| &form.key)
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
fn emblem(icon: IconName, dim: bool) {
    row_arranged(
        Modifier::new()
            .size(theme::sp(8), theme::sp(8))
            .background_rounded(theme::color(Role::SurfaceActive), Radius::Md.px()),
        Arrangement::Center,
        Alignment::Center,
        || glyph(icon, 4.5, if dim { Role::Subtle } else { Role::Text }),
    );
}

/// Emits a name `title` in `tone` followed by `badges`, over a quieter line `under` when it says something.
#[track_caller]
fn titled(title: &str, under: &str, tone: Role, badges: impl FnOnce()) {
    column_arranged(
        Modifier::new().weight(1.0).clip(),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    styled_text(
                        Modifier::new(),
                        title,
                        theme::text_style(13, 1.4)
                            .font_weight(rustle::FontWeight::MEDIUM)
                            .color(theme::color(tone))
                            .max_lines(1),
                    );
                    badges();
                },
            );
            if !under.is_empty() {
                line(under, 12, 1.4, Role::Muted);
            }
        },
    );
}

/// Emits the buttons `buttons` emits at the end of a row, side by side.
#[track_caller]
fn actions(buttons: impl FnOnce()) {
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        buttons,
    );
}

/// Emits a small button of a row saying `label` in the look of `variant`, answering to `id`.
#[track_caller]
fn small(label: &str, id: FocusId, variant: Variant) {
    Button::new(label, id)
        .variant(variant)
        .size(Size::Sm)
        .build();
}

/// Emits a small button showing only `icon`, saying what it does with `tooltip`, answering to `id`.
#[track_caller]
fn icon_button(icon: IconName, tooltip: &str, id: FocusId) {
    Button::icon(icon, id)
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .tooltip(tooltip)
        .build();
}

/// Emits a row of a boxed list, as the list's own rows are, set in from the left by `inset` steps and holding what `content` emits.
#[track_caller]
fn inset_row(inset: u32, content: impl FnOnce()) {
    row_arranged(
        Modifier::new()
            .padding_each(theme::sp(inset), 0.0, 0.0, 0.0)
            .padding_each(0.0, 0.0, theme::sp(3), 0.0)
            .padding_xy(0.0, theme::sp(3)),
        Arrangement::SpacedBy(sp(3)),
        Alignment::Center,
        content,
    );
}

/// Emits a row inside an agent's card, standing in to line up with the agent's name, holding what `content` emits.
#[track_caller]
fn inner_row(content: impl FnOnce()) {
    inset_row(ROW_INSET, content);
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
    flat_row(|| {
        row_arranged(
            Modifier::new().weight(1.0),
            Arrangement::SpacedBy(sp(3)),
            Alignment::Center,
            || {
                emblem(look.icon, !shown);
                titled(
                    name,
                    &under,
                    if shown { Role::Text } else { Role::Muted },
                    || {
                        if !shown {
                            Badge::new("Hidden").tone(Role::Subtle).build();
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

/// Emits the buttons every account has, by the names given for them: use it when it is not in use, sign it in, and set or forget its key.
#[track_caller]
fn account_buttons(
    signing: Signing,
    in_use: bool,
    status: (&str, bool),
    ids: [Option<FocusId>; 4],
) {
    let [use_id, sign_in, set_key, remove_key] = ids;
    let (said, signed_in) = status;
    let keyed_now = said.starts_with("API key");
    if signing.keyed {
        if let Some(remove) = remove_key.filter(|_| said == pm_harness::SAVED_KEY) {
            small("Remove key", remove, Variant::Ghost);
        }
        if let Some(set) = set_key {
            small(
                if keyed_now { "Replace key" } else { "Add key" },
                set,
                Variant::Ghost,
            );
        }
    }
    if signing.signs_in
        && let Some(sign_in) = sign_in
    {
        small(
            if signed_in && !keyed_now {
                "Sign in again"
            } else {
                "Sign in"
            },
            sign_in,
            Variant::Ghost,
        );
    }
    if let Some(use_id) = use_id.filter(|_| !in_use) {
        small("Use", use_id, Variant::Outline);
    }
}

/// Emits the box `id` an account's name or API key is typed into, holding `field` and showing `placeholder` while empty, with its buttons `save` and `cancel`, in place of the account's row.
#[track_caller]
fn form_row(
    field: &TextFieldState,
    id: FocusId,
    placeholder: &str,
    save: FocusId,
    cancel: FocusId,
) {
    let mut input = Input::new(field, id)
        .variant(crate::input::Variant::Bordered)
        .size(Size::Sm)
        .placeholder(placeholder)
        .modifier(Modifier::new().weight(1.0));
    if id == KEY_FIELD {
        input = input.leading_icon(IconName::Lock).secure(true);
    }
    rustle::key(id.0, || {
        inner_row(|| {
            input.build();
            actions(|| {
                small("Cancel", cancel, Variant::Ghost);
                small("Save", save, Variant::Default);
            });
        });
    });
}

/// The open key box, when it is for the account `login` of the agent at `index`, or its default account when `login` is nothing.
fn key_form_for<'a>(
    state: &'a WorkspaceState,
    index: usize,
    login: Option<&str>,
) -> Option<&'a KeyForm> {
    state
        .settings_servers
        .key_form
        .as_ref()
        .filter(|form| form.agent == index && form.login.as_deref() == login)
}

/// Emits the rows of the accounts of the agent at `index`, whose id is `id`: its default account, the ones added, each with who it is signed in as and its buttons, and the button adding one.
fn account_rows(state: &WorkspaceState, index: usize, id: &str, signing: Signing) {
    let in_use_badge = || Badge::new("In use").tone(Role::Subtle).build();
    let default_in_use = !state.in_use.iter().any(|(owner, _)| owner == id);
    let default_said = match state.defaults.get(index) {
        Some(Some(said)) => Some(said),
        _ => None,
    };
    let (status, signed_in) = status_of(default_said, signing);
    match key_form_for(state, index, None) {
        Some(form) => form_row(
            &form.key,
            KEY_FIELD,
            "Paste an API key",
            KEY_SAVE,
            KEY_CANCEL,
        ),
        None => inner_row(|| {
            titled(
                "Default account",
                &status,
                if signed_in { Role::Text } else { Role::Muted },
                || {
                    if default_in_use {
                        in_use_badge();
                    }
                },
            );
            actions(|| {
                account_buttons(
                    signing,
                    default_in_use,
                    (&status, signed_in),
                    [
                        DEFAULT_USES.id(index),
                        SIGN_INS.id(index),
                        KEY_SETS.id(index),
                        KEY_REMOVES.id(index),
                    ],
                );
            });
        }),
    }
    for (position, (_, login, name)) in state
        .logins
        .iter()
        .enumerate()
        .filter(|(_, (owner, _, _))| owner == id)
    {
        if let Some(form) = key_form_for(state, index, Some(login)) {
            form_row(
                &form.key,
                KEY_FIELD,
                "Paste an API key",
                KEY_SAVE,
                KEY_CANCEL,
            );
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
                NAME_FIELD,
                "Account name",
                NAME_SAVE,
                NAME_CANCEL,
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
        let (status, signed_in) = status_of(said, signing);
        rustle::key(position, || {
            inner_row(|| {
                titled(
                    name,
                    &status,
                    if signed_in { Role::Text } else { Role::Muted },
                    || {
                        if used {
                            in_use_badge();
                        }
                    },
                );
                actions(|| {
                    account_buttons(
                        signing,
                        used,
                        (&status, signed_in),
                        [
                            AccountControl::Use.id(position),
                            AccountControl::SignIn.id(position),
                            AccountControl::SetKey.id(position),
                            AccountControl::RemoveKey.id(position),
                        ],
                    );
                    if let Some(rename) = AccountControl::Rename.id(position) {
                        icon_button(IconName::Pencil, "Rename", rename);
                    }
                    if let Some(remove) = ACCOUNT_REMOVES.id(position) {
                        icon_button(IconName::Trash, "Remove account", remove);
                    }
                });
            });
        });
    }
    inset_row(ADD_INSET, || {
        if let Some(add) = ACCOUNT_ADDS.id(index) {
            Button::new("Add account", add)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .with_icon(IconName::Plus)
                .build();
        }
    });
}

/// Emits the card of the agent at `index`, `room` logical pixels across: its header, what the reader sets up for it when it needs something, then its accounts, each signed in or given a key of its own; nothing when there is no agent there.
#[track_caller]
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
            inner_row(|| {
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
pub(crate) fn is_key_field(id: FocusId) -> bool {
    id == KEY_FIELD || id == NAME_FIELD
}

/// The button the box `id` is kept with when Enter is pressed in it.
pub(crate) fn key_submit(id: FocusId) -> FocusId {
    if id == NAME_FIELD {
        NAME_SAVE
    } else {
        KEY_SAVE
    }
}

#[cfg(test)]
/// Tests for the agents page.
mod tests {
    use super::*;

    /// The position of the agent kept under `id`.
    fn position(state: &WorkspaceState, id: &str) -> usize {
        state.agent_position(id).unwrap()
    }

    /// Typing a key and saving it hands it to the server and forgets what the page said until it answers; a blank key is not sent, and removing sends nothing as the key.
    #[test]
    fn a_typed_key_is_sent_to_the_server() {
        let mut state = WorkspaceState::new(&[]);
        let openrouter = position(&state, "openrouter");
        assert!(state.agents_pressed(KEY_SETS.at(openrouter as u64)));
        assert!(
            state
                .key_field(KEY_FIELD)
                .is_some_and(|field| field.text().is_empty())
        );
        state = state.typed(KEY_FIELD, " sk-or-1 ");
        assert!(state.agents_pressed(KEY_SAVE));
        assert!(state.key_field(KEY_FIELD).is_none());
        let sent = state.take_outbox();
        assert!(sent.iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::SaveKey { agent, key: Some(key), .. } if *agent == openrouter && key == "sk-or-1"
        )));
        state.agents_pressed(KEY_SETS.at(openrouter as u64));
        state.agents_pressed(KEY_SAVE);
        assert!(state.take_outbox().is_empty());
        state.agents_pressed(KEY_REMOVES.at(openrouter as u64));
        assert!(
            state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::SaveKey { key: None, .. }))
        );
        assert!(!state.agents_pressed(FocusId(1)));
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
        state.agents_pressed(SIGN_INS.at(claude as u64));
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
        state = state.typed(NAME_FIELD, " Acme");
        assert!(state.agents_pressed(NAME_SAVE));
        assert_eq!(state.logins[0].2, "Account 2 Acme");
        state.take_outbox();
        assert!(state.agents_pressed(AccountControl::SignIn.id(0).unwrap()));
        assert!(state.agents_pressed(AccountControl::SetKey.id(0).unwrap()));
        state = state.typed(KEY_FIELD, "sk-2");
        assert!(state.agents_pressed(KEY_SAVE));
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
