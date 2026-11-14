//! The model servers page of the settings window: the OpenAI-compatible servers declared on the server's machine, each offered as an agent with its models, the form a server is added or changed with, and the models a server is found to offer, picked from to make its list.

use pm_harness::{ModelServer, ServerWire};
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_1, flex_col, flex_none, flex_wrap, gap,
    items_center, justify_between, justify_end, min_w_px, p, rounded,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::agent_list::AgentEntry;
use crate::button::Variant;
use crate::checkbox::Checkbox;
use crate::input::Input;
use crate::parts::{clipped, line};
use crate::settings_ids::{FOUND_MODEL_PICKS, MODEL_SERVER_EDITS, MODEL_SERVER_REMOVES, SINGLES};
use crate::settings_row::choices;
use crate::settings_servers::{boxed, heading_only, note, paragraph, wrapped_note};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Badge, Button};

/// Opens the form for declaring a model server.
pub const MODEL_ADD: FocusId = SINGLES.at(10);

/// Keeps what the form holds.
pub const MODEL_SAVE: FocusId = SINGLES.at(11);

/// Puts the form away without keeping it.
pub const MODEL_CANCEL: FocusId = SINGLES.at(12);

/// The box the server's name is written in.
pub const MODEL_NAME: FocusId = SINGLES.at(13);

/// The box the server's base URL is written in.
pub const MODEL_URL: FocusId = SINGLES.at(14);

/// The box the server's API key is written in; it never shows a key already saved.
pub const MODEL_KEY: FocusId = SINGLES.at(15);

/// The box naming the environment variables the server's key is read from.
pub const MODEL_KEY_ENV: FocusId = SINGLES.at(16);

/// The box the models the server offers are listed in, separated by commas.
pub const MODEL_LIST: FocusId = SINGLES.at(17);

/// Asks the server which models it offers.
pub const MODEL_FIND: FocusId = SINGLES.at(18);

/// Has the server spoken to through the Chat Completions API.
pub const MODEL_WIRE_CHAT: FocusId = SINGLES.at(19);

/// Has the server spoken to through the Responses API.
pub const MODEL_WIRE_RESPONSES: FocusId = SINGLES.at(20);

/// The protocols a server can speak, in the order the form offers them, with the names the form gives them.
const WIRES: [(ServerWire, &str); 2] = [
    (ServerWire::ChatCompletions, "Chat Completions"),
    (ServerWire::Responses, "Responses"),
];

/// The id a server is given when its name makes none.
const FALLBACK_ID: &str = "server";

/// What the model servers page keeps: the servers as the server holds them, the form while one is open, and why the last change could not be made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelServers {
    /// The servers declared, in the order they are offered, without their keys.
    pub servers: Vec<ModelServer>,
    /// The form for declaring or changing a server, while one is open.
    pub form: Option<ModelServerForm>,
    /// Why the servers could not be read or changed, when the last attempt failed.
    pub error: Option<String>,
    /// The ticket the next question about a server's models goes under.
    next_ticket: u64,
}

/// What asking a server for its models came to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    /// The ticket of the question waited for, while it is.
    pub asking: Option<u64>,
    /// The models the server offered at the last answer, in its order.
    pub models: Vec<String>,
    /// Why the server could not be asked, when it could not.
    pub error: Option<String>,
}

/// A model server being described: which one it replaces, the boxes it is written in, and the models it was found to offer.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelServerForm {
    /// The place in the list of the server being changed, or nothing for a new one.
    index: Option<usize>,
    /// The id of the server being changed, which stays as it was; nothing for a new one, which is given one from its name.
    id: Option<String>,
    /// What it is called.
    name: Field,
    /// The address `/chat/completions`, `/responses` and `/models` are appended to.
    base_url: Field,
    /// The protocol it speaks.
    wire: ServerWire,
    /// A new API key; left blank to keep the one saved.
    key: Field,
    /// The environment variables its key is read from, separated by commas or spaces.
    key_env: Field,
    /// The models it offers, separated by commas; blank to ask the server.
    models: Field,
    /// Whether a key is saved for it already.
    has_key: bool,
    /// What asking the server for its models came to.
    pub found: Found,
}

impl ModelServerForm {
    /// A form holding `server`, which is at `index` of the list, or an empty one for a new server.
    fn holding(index: Option<usize>, server: Option<&ModelServer>) -> Self {
        Self {
            index,
            id: server.map(|server| server.id.clone()),
            name: Field::new(
                server
                    .map(|server| server.label.clone().unwrap_or_else(|| server.id.clone()))
                    .unwrap_or_default(),
            ),
            base_url: Field::new(
                server
                    .map(|server| server.base_url.clone())
                    .unwrap_or_default(),
            ),
            wire: server.map(|server| server.wire).unwrap_or_default(),
            key: Field::default().masked(true),
            key_env: Field::new(
                server
                    .map(|server| server.api_key_env.join(", "))
                    .unwrap_or_default(),
            ),
            models: Field::new(
                server
                    .map(|server| server.models.join(", "))
                    .unwrap_or_default(),
            ),
            has_key: server.is_some_and(|server| server.has_api_key),
            found: Found::default(),
        }
    }

    /// The models the form lists, in order, each once.
    fn listed(&self) -> Vec<String> {
        let mut listed: Vec<String> = Vec::new();
        for model in words(self.models.text()) {
            if !listed.contains(&model) {
                listed.push(model);
            }
        }
        listed
    }

    /// Lists `model` in the form when it is not listed, or takes it out when it is.
    fn toggle_model(&mut self, model: &str) {
        let mut listed = self.listed();
        match listed.iter().position(|held| held == model) {
            Some(place) => {
                listed.remove(place);
            }
            None => listed.push(model.to_owned()),
        }
        self.models = Field::new(listed.join(", "));
    }

    /// The server the form describes under `id`, whether or not it has a name yet; nothing until it has an address.
    fn described(&self, id: String) -> Option<ModelServer> {
        let base_url = self.base_url.text().trim();
        if base_url.is_empty() {
            return None;
        }
        let name = self.name.text().trim();
        let key = self.key.text().trim();
        Some(ModelServer {
            id,
            label: (!name.is_empty()).then(|| name.to_owned()),
            base_url: base_url.to_owned(),
            wire: self.wire,
            api_key_env: words(self.key_env.text()),
            api_key: (!key.is_empty()).then(|| key.to_owned()),
            has_api_key: self.has_key,
            headers: Vec::new(),
            default_model: None,
            models: self.listed(),
            replay_reasoning: false,
        })
    }

    /// Whether the form holds enough to be kept: a name and an address.
    fn complete(&self) -> bool {
        !self.name.text().trim().is_empty() && !self.base_url.text().trim().is_empty()
    }
}

/// The words of `text`, which are separated by commas, spaces or line breaks.
fn words(text: &str) -> Vec<String> {
    text.split(|letter: char| letter == ',' || letter.is_whitespace())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The id a server called `name` is given: its name in lowercase, with every run of other letters made one `-`, and a number after it when `taken` has it already.
fn id_from(name: &str, taken: &[String]) -> String {
    let mut id = String::new();
    for letter in name.trim().chars().flat_map(char::to_lowercase) {
        if letter.is_ascii_alphanumeric() || letter == '_' {
            id.push(letter);
        } else if !id.is_empty() && !id.ends_with('-') {
            id.push('-');
        }
    }
    let stem = match id.trim_end_matches('-') {
        "" => FALLBACK_ID.to_owned(),
        stem => stem.to_owned(),
    };
    let mut candidate = stem.clone();
    let mut number = 2;
    while taken.contains(&candidate) {
        candidate = format!("{stem}-{number}");
        number += 1;
    }
    candidate
}

/// Whether `id` names a box of the form.
pub(crate) fn is_model_field(id: FocusId) -> bool {
    [MODEL_NAME, MODEL_URL, MODEL_KEY, MODEL_KEY_ENV, MODEL_LIST].contains(&id)
}

impl AgentEntry {
    /// The agents the app offers when `servers` are the model servers declared on the server: its built-in providers, then one agent for each server, under the server's id.
    pub fn with_model_servers(servers: &[ModelServer]) -> Vec<Self> {
        pm_harness::agents()
            .iter()
            .filter(|agent| !agent.server)
            .map(|agent| Self {
                id: agent.id.to_owned(),
                name: agent.name.to_owned(),
                server: false,
            })
            .chain(servers.iter().map(|server| Self {
                id: server.id.clone(),
                name: server.label.clone().unwrap_or_else(|| server.id.clone()),
                server: true,
            }))
            .collect()
    }
}

impl WorkspaceState {
    /// The state with `servers` as the model servers declared on the server: the page lists them, every one is offered as an agent, and what each offers is asked for.
    pub fn with_model_servers(mut self, servers: Vec<ModelServer>) -> Self {
        self.set_agents(AgentEntry::with_model_servers(&servers));
        self.settings_page.models.servers = servers;
        self.settings_page.models.error = None;
        self.ask_default_accounts();
        for agent in self.offered_agents() {
            if self
                .settings_servers
                .agents
                .get(agent)
                .is_some_and(|entry| entry.server)
            {
                self.outbox.push(Outgoing::Discover { agent, host: None });
            }
        }
        self
    }

    /// The state after the server could not read or change the model servers, saying why on the page.
    pub fn with_model_servers_failed(mut self, reason: String) -> Self {
        self.settings_page.models.error = Some(reason);
        self
    }

    /// The state with the models a server was found to offer, or why it could not be asked, when `ticket` is the question the form waits on.
    pub fn with_found_models(mut self, ticket: u64, answer: Result<Vec<String>, String>) -> Self {
        let Some(form) = self
            .settings_page
            .models
            .form
            .as_mut()
            .filter(|form| form.found.asking == Some(ticket))
        else {
            return self;
        };
        form.found.asking = None;
        match answer {
            Ok(models) => {
                form.found.models = models;
                form.found.error = None;
            }
            Err(reason) => {
                form.found.models = Vec::new();
                form.found.error = Some(reason);
            }
        }
        self
    }

    /// Carries out pressing `id` on the model servers page, and says whether it was something of it.
    pub(crate) fn models_pressed(&mut self, id: FocusId) -> bool {
        match id {
            MODEL_ADD => {
                self.settings_page.models.form = Some(ModelServerForm::holding(None, None))
            }
            MODEL_CANCEL => self.settings_page.models.form = None,
            MODEL_SAVE => self.save_model_server(),
            MODEL_FIND => self.find_models(),
            MODEL_WIRE_CHAT | MODEL_WIRE_RESPONSES => {
                if let Some(form) = self.settings_page.models.form.as_mut() {
                    form.wire = if id == MODEL_WIRE_CHAT {
                        ServerWire::ChatCompletions
                    } else {
                        ServerWire::Responses
                    };
                }
            }
            _ if let Some(place) = FOUND_MODEL_PICKS.index_of(id) => {
                if let Some(form) = self.settings_page.models.form.as_mut()
                    && let Some(model) = form.found.models.get(place).cloned()
                {
                    form.toggle_model(&model);
                }
            }
            _ if let Some(index) = MODEL_SERVER_EDITS.index_of(id) => {
                let models = &mut self.settings_page.models;
                models.form = models
                    .servers
                    .get(index)
                    .map(|server| ModelServerForm::holding(Some(index), Some(server)));
            }
            _ if let Some(index) = MODEL_SERVER_REMOVES.index_of(id) => {
                let models = &mut self.settings_page.models;
                if let Some(server) = models.servers.get(index) {
                    let id = server.id.clone();
                    models.form = None;
                    self.outbox.push(Outgoing::RemoveModelServer { id });
                }
            }
            _ => return false,
        }
        true
    }

    /// The ids the form's new server must not be given: the declared servers' and every agent's.
    fn taken_ids(&self) -> Vec<String> {
        self.settings_page
            .models
            .servers
            .iter()
            .map(|server| server.id.clone())
            .chain(
                pm_harness::agents()
                    .iter()
                    .map(|agent| agent.provider.to_owned()),
            )
            .chain(
                self.settings_servers
                    .agents
                    .iter()
                    .map(|agent| agent.id.clone()),
            )
            .collect()
    }

    /// The id of the server the form describes: the one it had, or one made from its name.
    fn form_id(&self, form: &ModelServerForm) -> String {
        form.id
            .clone()
            .unwrap_or_else(|| id_from(form.name.text(), &self.taken_ids()))
    }

    /// Has the server keep the server the form describes, and puts the form away; a form missing its name or address stays open.
    fn save_model_server(&mut self) {
        let Some(form) = self.settings_page.models.form.as_ref() else {
            return;
        };
        if !form.complete() {
            return;
        }
        let Some(server) = form.described(self.form_id(form)) else {
            return;
        };
        self.settings_page.models.form = None;
        self.outbox
            .push(Outgoing::SaveModelServer(Box::new(server)));
    }

    /// Asks the server the form describes which models it offers, whether or not it is saved; a form without an address asks nothing.
    fn find_models(&mut self) {
        let Some(form) = self.settings_page.models.form.as_ref() else {
            return;
        };
        let Some(mut server) = form.described(self.form_id(form)) else {
            return;
        };
        server.models = Vec::new();
        let models = &mut self.settings_page.models;
        let ticket = models.next_ticket;
        models.next_ticket += 1;
        if let Some(form) = models.form.as_mut() {
            form.found = Found {
                asking: Some(ticket),
                ..Found::default()
            };
        }
        self.outbox.push(Outgoing::FindModels {
            ticket,
            server: Box::new(server),
        });
    }

    /// Writes `edit` into the box `id` of the form.
    pub(crate) fn model_edited(&mut self, id: FocusId, edit: Edit) {
        let Some(form) = self.settings_page.models.form.as_mut() else {
            return;
        };
        match id {
            MODEL_NAME => form.name.apply(edit),
            MODEL_URL => form.base_url.apply(edit),
            MODEL_KEY => form.key.apply(edit),
            MODEL_KEY_ENV => form.key_env.apply(edit),
            MODEL_LIST => form.models.apply(edit),
            _ => {}
        }
    }

    /// The box `id` of the form, while it is open.
    pub(crate) fn model_field(&self, id: FocusId) -> Option<&Field> {
        let form = self.settings_page.models.form.as_ref()?;
        match id {
            MODEL_NAME => Some(&form.name),
            MODEL_URL => Some(&form.base_url),
            MODEL_KEY => Some(&form.key),
            MODEL_KEY_ENV => Some(&form.key_env),
            MODEL_LIST => Some(&form.models),
            _ => None,
        }
    }
}

/// A box of the form under the words that say what is written in it.
fn labelled(label: &str, field: &Field, id: FocusId, placeholder: &str) -> Node {
    node()
        .styles([flex(), flex_col(), gap(1)])
        .child(line(label, 12, 1.4, Role::Muted))
        .child(
            Input::new(field, id)
                .size(Size::Sm)
                .placeholder(placeholder)
                .build(),
        )
}

/// The models the server was found to offer, each with a box that lists it in the form or takes it out, or what asking it came to.
fn found_node(form: &ModelServerForm, room: f32) -> Option<Node> {
    let found = &form.found;
    if found.asking.is_some() {
        return Some(note("Asking the server for its models..."));
    }
    if let Some(reason) = &found.error {
        return Some(wrapped_note(
            &format!("The server could not be asked: {reason}"),
            room,
        ));
    }
    if found.models.is_empty() {
        return None;
    }
    let listed = form.listed();
    let picks = found
        .models
        .iter()
        .enumerate()
        .filter_map(|(place, model)| {
            let pick = FOUND_MODEL_PICKS.id(place)?;
            Some(
                node()
                    .styles([flex(), items_center(), gap(2), flex_none()])
                    .child(Checkbox::new(listed.contains(model), pick).build())
                    .child(
                        line(model.clone(), 12, 1.4, Role::Text).styles([pm_style::font_mono()]),
                    ),
            )
        });
    Some(
        node()
            .styles([flex(), flex_col(), gap(2)])
            .child(wrapped_note(
                &format!(
                    "Found {} models; tick the ones to offer, or leave the list blank to offer every one.",
                    found.models.len()
                ),
                (room - 48.0).max(160.0),
            ))
            .child(
                node()
                    .styles([flex(), flex_wrap(), gap(3), p(3), rounded(Radius::Md), border(), border_color(Role::Border), bg(Role::Background)])
                    .children(picks),
            ),
    )
}

/// The form a model server is declared or changed with, `room` logical pixels across.
fn form_node(form: &ModelServerForm, room: f32) -> Node {
    let wire = choices(
        &WIRES.map(|(_, name)| name),
        WIRES
            .iter()
            .position(|(held, _)| *held == form.wire)
            .unwrap_or(0),
        |option| [MODEL_WIRE_CHAT, MODEL_WIRE_RESPONSES].get(option).copied(),
    );
    let key_hint = if form.has_key {
        "Key saved; leave blank to keep it"
    } else {
        "sk-..."
    };
    node()
        .styles([flex(), flex_col(), gap(3), p(3)])
        .child(labelled(
            "Name",
            &form.name,
            MODEL_NAME,
            "e.g. Company gateway",
        ))
        .child(labelled(
            "Base URL",
            &form.base_url,
            MODEL_URL,
            "e.g. http://localhost:8000/v1",
        ))
        .child(
            node()
                .styles([flex(), flex_col(), gap(1)])
                .child(line("API", 12, 1.4, Role::Muted))
                .child(node().styles([flex()]).child(wire)),
        )
        .child(labelled("API key", &form.key, MODEL_KEY, key_hint))
        .child(labelled(
            "Or read the key from (environment variables)",
            &form.key_env,
            MODEL_KEY_ENV,
            "e.g. GATEWAY_API_KEY",
        ))
        .child(labelled(
            "Models (separated by commas; blank asks the server)",
            &form.models,
            MODEL_LIST,
            "e.g. qwen3-coder, gpt-oss-120b",
        ))
        .children(found_node(form, room))
        .child(
            node()
                .styles([flex(), items_center(), justify_between(), gap(2)])
                .child(
                    Button::new("Find models", MODEL_FIND)
                        .variant(Variant::Outline)
                        .size(Size::Sm)
                        .with_icon(IconName::Search)
                        .disabled(
                            form.base_url.text().trim().is_empty() || form.found.asking.is_some(),
                        )
                        .build(),
                )
                .child(
                    node()
                        .styles([flex(), items_center(), justify_end(), gap(2)])
                        .child(
                            Button::new("Cancel", MODEL_CANCEL)
                                .variant(Variant::Ghost)
                                .size(Size::Sm)
                                .build(),
                        )
                        .child(
                            Button::new("Save", MODEL_SAVE)
                                .size(Size::Sm)
                                .disabled(!form.complete())
                                .build(),
                        ),
                ),
        )
}

/// How `server` is reached and what it offers, in the words under its name.
fn detail_of(server: &ModelServer) -> String {
    let wire = match server.wire {
        ServerWire::ChatCompletions => "Chat Completions",
        ServerWire::Responses => "Responses",
    };
    let models = match server.models.len() {
        0 => "models asked of the server".to_owned(),
        1 => "1 model".to_owned(),
        many => format!("{many} models"),
    };
    format!("{wire} · {} · {models}", server.base_url)
}

/// How `server` is given its key, as a label beside its name: a key saved, the variables it reads, or nothing.
fn key_badge(server: &ModelServer) -> Option<String> {
    if server.has_api_key {
        Some("Key saved".to_owned())
    } else {
        server
            .api_key_env
            .first()
            .map(|variable| format!("Key from {variable}"))
    }
}

/// The row of the server at `index`: its name and how its key is given, over how it is reached and its models, with its buttons.
fn server_row(index: usize, server: &ModelServer) -> Node {
    let name = server.label.clone().unwrap_or_else(|| server.id.clone());
    let words = node()
        .styles([flex(), flex_col(), gap(0.5), flex_1(), min_w_px(0)])
        .child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(line(name, 13, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .children(key_badge(server).map(|label| Badge::new(label).build())),
        )
        .child(clipped(detail_of(server), 12, Role::Muted))
        .children((!server.models.is_empty()).then(|| {
            clipped(server.models.join(", "), 11, Role::Subtle).styles([pm_style::font_mono()])
        }));
    node()
        .styles([
            flex(),
            items_center(),
            justify_between(),
            gap(3),
            flex_none(),
            pm_style::px(3),
            pm_style::py(3),
        ])
        .child(words)
        .child(
            node()
                .styles([flex(), items_center(), gap(1), flex_none()])
                .children(MODEL_SERVER_EDITS.id(index).map(|edit| {
                    Button::new("Edit", edit)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build()
                }))
                .children(MODEL_SERVER_REMOVES.id(index).map(|remove| {
                    Button::new("Remove", remove)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build()
                })),
        )
}

/// What the page says about itself under its heading.
pub(crate) const SUMMARY: &str = "OpenAI-compatible servers, such as a company gateway or a local vLLM, LM Studio or llama.cpp server. Each one is offered as an agent, with its models in the model picker.";

/// The body of the model servers page, `room` logical pixels across: what it is for, then every server with the form while it is open.
pub(crate) fn body(state: &WorkspaceState, page: Node, room: f32) -> Node {
    let models = &state.settings_page.models;
    let form = models.form.as_ref();
    let adding = form.is_some_and(|form| form.index.is_none());
    let mut rows: Vec<Node> = models
        .servers
        .iter()
        .enumerate()
        .map(|(index, server)| match form {
            Some(form) if form.index == Some(index) => form_node(form, room),
            _ => server_row(index, server),
        })
        .collect();
    if let (Some(form), true) = (form, adding) {
        rows.push(form_node(form, room));
    }
    let page =
        page.child(paragraph(SUMMARY, room))
            .children(models.error.as_deref().map(|reason| {
                wrapped_note(&format!("Could not change the servers: {reason}"), room)
            }))
            .child(
                node()
                    .styles([flex(), items_center(), justify_between(), flex_none()])
                    .child(heading_only("Servers", models.servers.len()))
                    .child(
                        Button::new("Add Server", MODEL_ADD)
                            .variant(Variant::Outline)
                            .size(Size::Sm)
                            .disabled(adding)
                            .build(),
                    ),
            );
    if rows.is_empty() {
        page.child(note("No model servers are declared."))
    } else {
        page.child(boxed(rows))
    }
}

/// How many servers the page lists, in words, for its card on the Agents overview.
pub(crate) fn count(state: &WorkspaceState) -> String {
    format!("{} declared", state.settings_page.models.servers.len())
}

#[cfg(test)]
/// Tests for the model servers page: declaring, changing and removing a server, and finding its models.
mod tests {
    use super::*;
    use crate::settings::Section;

    /// A saved server with a key, whose models are asked of it.
    fn saved() -> ModelServer {
        ModelServer {
            id: "gateway".to_owned(),
            label: Some("Gateway".to_owned()),
            base_url: "http://localhost:8000/v1".to_owned(),
            wire: ServerWire::ChatCompletions,
            api_key_env: Vec::new(),
            api_key: None,
            has_api_key: true,
            headers: Vec::new(),
            default_model: None,
            models: Vec::new(),
            replay_reasoning: false,
        }
    }

    /// The state with the settings window open on the model servers page, listing `servers`, and nothing waiting to be sent.
    fn open_with(servers: Vec<ModelServer>) -> WorkspaceState {
        let mut state = WorkspaceState::new(&[])
            .with_settings()
            .settings_pressed(Section::ModelServers.id())
            .with_model_servers(servers);
        state.take_outbox();
        state
    }

    /// Writes `text` into the box `id`.
    fn typed(state: WorkspaceState, id: FocusId, text: &str) -> WorkspaceState {
        state.edited(id, Edit::Paste(text.to_owned()))
    }

    /// Opening the page asks the server for its model servers.
    #[test]
    fn opening_the_page_asks_for_the_servers() {
        let mut state = WorkspaceState::new(&[]).with_settings();
        state.take_outbox();
        let mut state = state.settings_pressed(Section::ModelServers.id());
        assert!(
            state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::ModelServers))
        );
    }

    /// A server declared with a name, an address, the Responses API and a key is sent to be saved under an id made from its name, with its key, and the form goes away.
    #[test]
    fn a_new_server_is_saved_under_an_id_from_its_name() {
        let state = open_with(Vec::new()).settings_pressed(MODEL_ADD);
        let state = typed(state, MODEL_NAME, "Lab Box!");
        let state = typed(state, MODEL_URL, "http://lab:8000/v1");
        let state = typed(state, MODEL_KEY, "sk-secret");
        let state = typed(state, MODEL_LIST, "qwen3-coder, gpt-oss  qwen3-coder");
        let mut state = state
            .settings_pressed(MODEL_WIRE_RESPONSES)
            .settings_pressed(MODEL_SAVE);
        assert!(state.settings_page.models.form.is_none());
        let sent = state.take_outbox();
        let Some(Outgoing::SaveModelServer(server)) = sent
            .iter()
            .find(|outgoing| matches!(outgoing, Outgoing::SaveModelServer(_)))
        else {
            panic!("nothing was saved: {sent:?}");
        };
        assert_eq!(server.id, "lab-box");
        assert_eq!(server.label.as_deref(), Some("Lab Box!"));
        assert_eq!(server.wire, ServerWire::Responses);
        assert_eq!(server.api_key.as_deref(), Some("sk-secret"));
        assert_eq!(server.models, ["qwen3-coder", "gpt-oss"]);
    }

    /// A form missing its address is not saved and stays open.
    #[test]
    fn an_incomplete_server_is_not_saved() {
        let state = open_with(Vec::new()).settings_pressed(MODEL_ADD);
        let mut state = typed(state, MODEL_NAME, "Box").settings_pressed(MODEL_SAVE);
        assert!(state.settings_page.models.form.is_some());
        assert!(
            !state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::SaveModelServer(_)))
        );
    }

    /// Editing a server never shows its saved key, keeps its id, and sends no key when the box is left blank, so the saved one stays.
    #[test]
    fn editing_keeps_the_id_and_the_saved_key() {
        let state = open_with(vec![saved()]).settings_pressed(MODEL_SERVER_EDITS.at(0));
        assert_eq!(state.field(MODEL_KEY).map(Field::text), Some(""));
        assert_eq!(state.field(MODEL_NAME).map(Field::text), Some("Gateway"));
        let mut state = typed(state, MODEL_NAME, " Two").settings_pressed(MODEL_SAVE);
        let sent = state.take_outbox();
        let Some(Outgoing::SaveModelServer(server)) = sent.first() else {
            panic!("nothing was saved: {sent:?}");
        };
        assert_eq!(server.id, "gateway");
        assert_eq!(server.label.as_deref(), Some("Gateway Two"));
        assert_eq!(server.api_key, None);
    }

    /// Removing a server asks the server to take it out by its id.
    #[test]
    fn removing_sends_the_id() {
        let mut state = open_with(vec![saved()]).settings_pressed(MODEL_SERVER_REMOVES.at(0));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::RemoveModelServer { id } if id == "gateway"
        )));
    }

    /// Finding models asks the server under a ticket with its list left out, the answer to that ticket is shown, and ticking a found model lists it in the form; an answer to another ticket is ignored.
    #[test]
    fn found_models_are_picked_into_the_list() {
        let state = open_with(vec![saved()]).settings_pressed(MODEL_SERVER_EDITS.at(0));
        let mut state = typed(state, MODEL_LIST, "kept").settings_pressed(MODEL_FIND);
        let sent = state.take_outbox();
        let Some(Outgoing::FindModels { ticket, server }) = sent.first() else {
            panic!("no models were asked for: {sent:?}");
        };
        assert!(server.models.is_empty());
        assert_eq!(server.id, "gateway");
        let ticket = *ticket;
        let state = state.with_found_models(ticket + 1, Ok(vec!["stale".to_owned()]));
        let form = state.settings_page.models.form.as_ref().unwrap();
        assert_eq!(form.found.asking, Some(ticket));
        let state = state.with_found_models(ticket, Ok(vec!["a".to_owned(), "b".to_owned()]));
        let state = state
            .settings_pressed(FOUND_MODEL_PICKS.at(1))
            .settings_pressed(FOUND_MODEL_PICKS.at(1))
            .settings_pressed(FOUND_MODEL_PICKS.at(0));
        assert_eq!(state.field(MODEL_LIST).map(Field::text), Some("kept, a"));
        let state = state.with_found_models(ticket, Err("late".to_owned()));
        assert!(
            state
                .settings_page
                .models
                .form
                .as_ref()
                .unwrap()
                .found
                .error
                .is_none()
        );
    }

    /// A server that could not be asked says why.
    #[test]
    fn a_failed_search_says_why() {
        let state = open_with(vec![saved()])
            .settings_pressed(MODEL_SERVER_EDITS.at(0))
            .settings_pressed(MODEL_FIND);
        let state = state.with_found_models(0, Err("connection refused".to_owned()));
        let found = &state.settings_page.models.form.as_ref().unwrap().found;
        assert_eq!(found.error.as_deref(), Some("connection refused"));
        assert!(found.models.is_empty());
    }

    /// The declared servers are offered as agents after the built-in ones, named by their labels, and are asked what they offer.
    #[test]
    fn declared_servers_become_agents() {
        let mut state = WorkspaceState::new(&[]).with_model_servers(vec![saved()]);
        let last = state.settings_servers.agents.last().unwrap().clone();
        assert_eq!(last.id, "gateway");
        assert_eq!(last.name, "Gateway");
        assert!(last.server);
        let place = state.agent_position("gateway").unwrap();
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::Discover { agent, host: None } if *agent == place
        )));
        let state = state.with_model_servers(Vec::new());
        assert!(state.agent_position("gateway").is_none());
    }

    /// An id made from a name is lowercase with its other letters made dashes, and numbered when taken.
    #[test]
    fn ids_are_made_from_names() {
        assert_eq!(id_from("My  Gateway", &[]), "my-gateway");
        assert_eq!(id_from("!!", &[]), "server");
        assert_eq!(id_from("openai", &["openai".to_owned()]), "openai-2");
    }
}
