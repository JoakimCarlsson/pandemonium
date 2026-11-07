//! The page of the agents and their accounts: a card for each agent with how it signs in, its switch, and its accounts or its API key.

use pm_harness::SignIn;
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, flex, flex_1, flex_col, flex_none, gap, items_center, justify_center,
    min_w_px, overflow_hidden, pl, rounded, size,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::input::Input;
use crate::parts::{clipped, glyph, line};
use crate::settings::{
    SETTINGS_ACCOUNT_ADD_BASE, SETTINGS_ACCOUNT_REMOVE_BASE, SETTINGS_SWITCH_BASE,
};
use crate::settings_row::switch;
use crate::settings_servers::{boxed, flat_row, paragraph};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Badge, Button};

/// The box an API key is typed into.
pub const KEY_FIELD: FocusId = FocusId(20_000_090);

/// Keeps the API key typed.
pub const KEY_SAVE: FocusId = FocusId(20_000_091);

/// Puts the box for an API key away.
pub const KEY_CANCEL: FocusId = FocusId(20_000_092);

/// The first of the buttons opening the box for an agent's API key, by the agent's position; the rest follow it.
pub const KEY_SET_BASE: u64 = 20_005_400;

/// The first of the buttons forgetting an agent's saved API key, by the agent's position; the rest follow it.
const KEY_REMOVE_BASE: u64 = 20_005_430;

/// The first of the buttons signing an agent's default account in, by the agent's position; the rest follow it.
const SIGN_IN_BASE: u64 = 20_005_460;

/// One past the last of the buttons of the page that follow a base.
const AGENT_BUTTONS_END: u64 = 20_005_490;

/// How many agents the buttons that follow a base have room for.
const AGENT_ROOM: u64 = 30;

/// How far an agent's rows stand in, so that they line up with its name, in spacing steps.
const ROW_INSET: u32 = 11;

/// How far the row adding an account stands in, so that its button's glyph lines up with the names above, in spacing steps.
const ADD_INSET: u32 = 8;

/// The box for an agent's API key while it is open.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyForm {
    /// The agent, by position.
    agent: usize,
    /// The key being typed, shown as dots.
    key: Field,
}

impl WorkspaceState {
    /// Carries out `action` if it presses a part of the agents page, and says whether it did.
    pub(crate) fn agents_pressed(&mut self, action: FocusId) -> bool {
        let FocusId(number) = action;
        let at = |base: u64| {
            (base..base + AGENT_ROOM)
                .contains(&number)
                .then(|| (number - base) as usize)
        };
        if action == KEY_SAVE {
            self.save_key();
        } else if action == KEY_CANCEL {
            self.settings_servers.key_form = None;
        } else if let Some(agent) = at(KEY_SET_BASE) {
            self.settings_servers.key_form = Some(KeyForm {
                agent,
                key: Field::new("").masked(true),
            });
        } else if let Some(agent) = at(KEY_REMOVE_BASE) {
            self.send_key(agent, None);
        } else if let Some(agent) = at(SIGN_IN_BASE) {
            self.outbox.push(Outgoing::SignIn { agent, login: None });
        } else {
            return (KEY_SET_BASE..AGENT_BUTTONS_END).contains(&number);
        }
        true
    }

    /// Hands the server the API key typed in the open box, and puts the box away; a blank box is put away only.
    fn save_key(&mut self) {
        let Some(form) = self.settings_servers.key_form.take() else {
            return;
        };
        let key = form.key.text().trim().to_owned();
        if !key.is_empty() {
            self.send_key(form.agent, Some(key));
        }
    }

    /// Has the server keep `key` as the API key of the agent at `agent`, or forget the saved one, and forgets what the page said about it until the server answers.
    fn send_key(&mut self, agent: usize, key: Option<String>) {
        if let Some(held) = self.defaults.get_mut(agent) {
            *held = None;
        }
        self.outbox.push(Outgoing::SaveKey { agent, key });
    }

    /// The state after `edit` was made to the box for an API key.
    pub(crate) fn key_edited(&mut self, edit: Edit) {
        if let Some(form) = self.settings_servers.key_form.as_mut() {
            form.key.apply(edit);
        }
    }

    /// The box for an API key, while it is open.
    pub(crate) fn key_field(&self) -> Option<&Field> {
        self.settings_servers
            .key_form
            .as_ref()
            .map(|form| &form.key)
    }
}

/// How the agent at `agent` signs in, in the words under its name.
fn how_it_signs_in(sign_in: Option<SignIn>) -> &'static str {
    match sign_in {
        Some(SignIn::Browser) => "Signs in through your browser",
        Some(SignIn::DeviceCode) => "Signs in with a one-time code",
        Some(SignIn::ApiKey) => "Uses an API key",
        Some(SignIn::None) => "Runs on a local server, no sign-in needed",
        None => "",
    }
}

/// The square an agent's glyph sits in at the start of its card.
fn emblem(icon: IconName, dim: bool) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            justify_center(),
            flex_none(),
            size(8),
            rounded(Radius::Md),
            bg(Role::SurfaceActive),
        ])
        .child(glyph(
            icon,
            4.5,
            if dim { Role::Subtle } else { Role::Text },
        ))
}

/// A name over a quieter line, side by side with nothing else.
fn titled(title: &str, under: &str, tone: Role, badges: Vec<Node>) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            gap(1),
            flex_1(),
            min_w_px(0),
            overflow_hidden(),
        ])
        .child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(line(title, 13, 1.4, tone).styles([pm_style::font_medium()]))
                .children(badges),
        )
        .children((!under.is_empty()).then(|| clipped(under, 12, Role::Muted)))
}

/// The buttons at the end of a row, side by side.
fn actions(buttons: Vec<Node>) -> Node {
    node()
        .styles([flex(), items_center(), gap(2), flex_none()])
        .children(buttons)
}

/// A small button of a row.
fn small(label: &str, id: FocusId, variant: Variant) -> Node {
    Button::new(label, id)
        .variant(variant)
        .size(Size::Sm)
        .build()
}

/// A row inside an agent's card, standing in to line up with the agent's name.
fn inner_row() -> Node {
    flat_row().styles([pl(ROW_INSET)])
}

/// The top of an agent's card: its glyph, its name and how it signs in, and the switch that offers it for new sessions.
fn header(state: &WorkspaceState, index: usize, name: &str, sign_in: Option<SignIn>) -> Node {
    let shown = state.agent_shown(index);
    let look = state.agent_look(index);
    let badges = if shown {
        Vec::new()
    } else {
        vec![Badge::new("Hidden").tone(Role::Subtle).build()]
    };
    flat_row()
        .child(
            node()
                .styles([flex(), items_center(), gap(3), flex_1(), min_w_px(0)])
                .child(emblem(look.icon, !shown))
                .child(titled(
                    name,
                    how_it_signs_in(sign_in),
                    if shown { Role::Text } else { Role::Muted },
                    badges,
                )),
        )
        .child(switch(shown, FocusId(SETTINGS_SWITCH_BASE + index as u64)))
}

/// The rows of the accounts of the agent at `index`: its default account, the ones added, and the button adding one.
fn account_rows(state: &WorkspaceState, index: usize, id: &str) -> Vec<Node> {
    let default_in_use = !state.in_use.iter().any(|(owner, _)| owner == id);
    let in_use = || Badge::new("In use").tone(Role::Subtle).build();
    let signed_in = match state.defaults.get(index) {
        Some(Some(Some(name))) => Some(name.clone()),
        _ => None,
    };
    let checking = !matches!(state.defaults.get(index), Some(Some(_)));
    let default_title = match (&signed_in, checking) {
        (Some(name), _) => name.clone(),
        (None, true) => "Checking…".to_owned(),
        (None, false) => "Not signed in".to_owned(),
    };
    let default_buttons = if signed_in.is_none() && !checking && default_in_use {
        vec![small(
            "Sign in",
            FocusId(SIGN_IN_BASE + index as u64),
            Variant::Outline,
        )]
    } else {
        Vec::new()
    };
    let default = inner_row()
        .child(titled(
            &default_title,
            "Default account",
            if signed_in.is_some() {
                Role::Text
            } else {
                Role::Muted
            },
            if default_in_use {
                vec![in_use()]
            } else {
                Vec::new()
            },
        ))
        .child(actions(default_buttons));
    let added = state
        .logins
        .iter()
        .enumerate()
        .filter(|(_, (owner, _, _))| owner == id)
        .map(|(position, (_, login, name))| {
            let used = state
                .in_use
                .iter()
                .any(|(owner, held)| owner == id && held == login);
            inner_row()
                .child(titled(
                    name,
                    "Added account",
                    Role::Text,
                    if used { vec![in_use()] } else { Vec::new() },
                ))
                .child(actions(vec![small(
                    "Remove",
                    FocusId(SETTINGS_ACCOUNT_REMOVE_BASE + position as u64),
                    Variant::Ghost,
                )]))
        });
    let add = flat_row().styles([pl(ADD_INSET)]).child(
        Button::new(
            "Add account",
            FocusId(SETTINGS_ACCOUNT_ADD_BASE + index as u64),
        )
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .with_icon(IconName::Plus)
        .build(),
    );
    std::iter::once(default)
        .chain(added)
        .chain(std::iter::once(add))
        .collect()
}

/// The row of the API key of the agent at `index`: where its key comes from and the buttons that set or forget it, or the box it is typed into.
fn key_row(state: &WorkspaceState, index: usize, variables: &[String]) -> Node {
    if let Some(form) = state
        .settings_servers
        .key_form
        .as_ref()
        .filter(|form| form.agent == index)
    {
        return inner_row()
            .child(
                node().styles([flex_1(), min_w_px(0)]).child(
                    Input::new(&form.key, KEY_FIELD)
                        .variant(crate::input::Variant::Bordered)
                        .size(Size::Sm)
                        .leading_icon(IconName::Lock)
                        .placeholder("Paste an API key")
                        .build(),
                ),
            )
            .child(actions(vec![
                small("Cancel", KEY_CANCEL, Variant::Ghost),
                small("Save", KEY_SAVE, Variant::Default),
            ]));
    }
    let said = match state.defaults.get(index) {
        Some(Some(Some(said))) => Some(said.as_str()),
        Some(Some(None)) => None,
        _ => Some("Checking…"),
    };
    let saved = said == Some(pm_harness::SAVED_KEY);
    let hint = match variables.first() {
        Some(variable) => format!("Saved on this machine, or read from {variable}"),
        None => "Saved on this machine".to_owned(),
    };
    let mut buttons = Vec::new();
    if saved {
        buttons.push(small(
            "Remove",
            FocusId(KEY_REMOVE_BASE + index as u64),
            Variant::Ghost,
        ));
    }
    buttons.push(small(
        if saved { "Replace key" } else { "Add key" },
        FocusId(KEY_SET_BASE + index as u64),
        Variant::Outline,
    ));
    inner_row()
        .child(titled(
            said.unwrap_or("No API key"),
            &hint,
            if said.is_some() {
                Role::Text
            } else {
                Role::Muted
            },
            Vec::new(),
        ))
        .child(actions(buttons))
}

/// The card of the agent at `index`: its header, then its accounts or its key.
fn agent_card(state: &WorkspaceState, index: usize) -> Option<Node> {
    let entry = state.settings_servers.agents.get(index)?;
    let agent = pm_harness::agent(&entry.id);
    let sign_in = agent.map(|agent| agent.sign_in);
    let mut rows = vec![header(state, index, &entry.name, sign_in)];
    match sign_in {
        Some(SignIn::Browser | SignIn::DeviceCode) => {
            rows.extend(account_rows(state, index, &entry.id));
        }
        Some(SignIn::ApiKey) => rows.push(key_row(
            state,
            index,
            agent.map_or(&[], |agent| agent.key_env),
        )),
        _ => {}
    }
    Some(boxed(rows))
}

/// The page of the agents: what it is for, then a card for each agent.
pub(crate) fn body(state: &WorkspaceState, page: Node, room: f32) -> Node {
    page.child(paragraph(
        "Every agent the harness offers. New sessions offer the ones signed in; switch one off to leave it out even then. Its sessions and accounts stay.",
        room,
    ))
    .children((0..state.agent_slots()).filter_map(|index| agent_card(state, index)))
}

/// Whether `id` names the box for an API key.
pub(crate) fn is_key_field(id: FocusId) -> bool {
    id == KEY_FIELD
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
        assert!(state.agents_pressed(FocusId(KEY_SET_BASE + openrouter as u64)));
        assert!(
            state
                .key_field()
                .is_some_and(|field| field.text().is_empty())
        );
        state.key_edited(Edit::Paste(" sk-or-1 ".to_owned()));
        assert!(state.agents_pressed(KEY_SAVE));
        assert!(state.key_field().is_none());
        let sent = state.take_outbox();
        assert!(sent.iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::SaveKey { agent, key: Some(key) } if *agent == openrouter && key == "sk-or-1"
        )));
        state.agents_pressed(FocusId(KEY_SET_BASE + openrouter as u64));
        state.agents_pressed(KEY_SAVE);
        assert!(state.take_outbox().is_empty());
        state.agents_pressed(FocusId(KEY_REMOVE_BASE + openrouter as u64));
        assert!(
            state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::SaveKey { key: None, .. }))
        );
        assert!(!state.agents_pressed(FocusId(1)));
    }

    /// The default account can be signed in from its row.
    #[test]
    fn the_default_account_signs_in_from_its_row() {
        let mut state = WorkspaceState::new(&[]);
        let claude = position(&state, "claude-code");
        state.agents_pressed(FocusId(SIGN_IN_BASE + claude as u64));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::SignIn { agent, login: None } if *agent == claude
        )));
    }
}
