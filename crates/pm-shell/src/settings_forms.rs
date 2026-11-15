//! The forms of the settings window's sections inside Agents: an agent's
//! key, sign-in and accounts; the tool server registry and the form adding
//! a tool server; the form declaring a model server; the slot being edited;
//! and the draft of a new customization. Each keeps what is typed until it
//! is sent, and takes in what the server answers.

use pm_protocol::library::{Kind, Scope};
use pm_protocol::{Login, McpServer, Request, Response, ToolListing};

use crate::settings_model::SettingsModel;

/// Something typed or pressed in a form of the settings window.
#[derive(Clone, Debug, PartialEq)]
pub enum FormChange {
    /// Opens the key, sign-in and accounts of the agent of this id, or closes them when nothing.
    OpenAgent(Option<String>),
    /// Holds the key typed for the open agent.
    KeyText(String),
    /// Saves the key typed for the open agent.
    SaveKey,
    /// Forgets the key saved for the open agent.
    ForgetKey,
    /// Signs the open agent in, as the account of this id or its own login when nothing.
    SignIn(Option<String>),
    /// Holds the name typed for a new account of the open agent.
    AccountName(String),
    /// Adds an account of the typed name to the open agent.
    AddAccount,
    /// Has the open agent start new sessions as the account of this id.
    UseAccount(String),
    /// Takes the account of this id away.
    RemoveAccount(String),
    /// Holds what the tool server registry is searched for.
    RegistryQuery(String),
    /// Searches the registry for what is typed.
    SearchRegistry,
    /// Installs the registry's listing at this place.
    InstallListing(usize),
    /// Opens the form adding a tool server, or closes it.
    ToggleMcpForm,
    /// Holds a field of the tool server form.
    McpField(McpField, String),
    /// Adds the tool server the form describes.
    AddMcp,
    /// Opens the form declaring a model server, or closes it.
    ToggleModelForm,
    /// Holds a field of the model server form.
    ModelField(ModelField, String),
    /// Asks the model server the form describes which models it offers.
    FindModels,
    /// Declares the model server the form describes.
    SaveModelServer,
    /// Opens the slot of this name for editing, or closes it when nothing.
    EditSlot(Option<String>),
    /// Holds the model typed for the slot being edited.
    SlotModel(String),
    /// Holds the backups typed for the slot being edited, separated by commas.
    SlotBackups(String),
    /// Sets the slot being edited to what is typed.
    SetSlot,
    /// Takes the reader's assignment of the slot of this name out, so it inherits again.
    ClearSlot(String),
    /// Puts the slots of the preset of this name in place.
    ApplyPreset(String),
    /// Opens the draft of a new customization of this kind, or closes it when nothing.
    OpenDraft(Option<Kind>),
    /// Holds the name of the draft.
    DraftName(String),
    /// Holds what the draft is for.
    DraftDescription(String),
    /// Keeps the draft in the reader's own folder when true, the project's otherwise.
    DraftPersonal(bool),
    /// Makes the customization the draft describes.
    CreateDraft,
}

/// A field of the tool server form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpField {
    /// What it is called.
    Name,
    /// The command that starts it, or its address.
    Target,
    /// Its variables, as `NAME=value` separated by commas.
    Env,
}

/// A field of the model server form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelField {
    /// The id it is offered under.
    Id,
    /// What it is called.
    Label,
    /// Its base address.
    BaseUrl,
    /// Its key.
    Key,
    /// The model offered first.
    DefaultModel,
}

/// The tool server form.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpForm {
    /// What it is called.
    pub name: String,
    /// The command that starts it, or its address.
    pub target: String,
    /// Its variables, as `NAME=value` separated by commas.
    pub env: String,
}

/// The model server form.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelForm {
    /// The id it is offered under.
    pub id: String,
    /// What it is called.
    pub label: String,
    /// Its base address.
    pub base_url: String,
    /// Its key, empty to keep none.
    pub key: String,
    /// The model offered first.
    pub default_model: String,
    /// The models it offers, once asked.
    pub models: Vec<String>,
    /// Whether asking it for its models is under way.
    pub finding: bool,
}

/// The draft of a new customization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    /// What it is.
    pub kind: Kind,
    /// Its name: lowercase letters, digits and hyphens.
    pub name: String,
    /// What it is for.
    pub description: String,
    /// Whether it goes in the reader's own folder rather than the project's.
    pub personal: bool,
}

/// What the forms of the settings window hold.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsForms {
    /// The agent whose key, sign-in and accounts are open.
    pub agent: Option<String>,
    /// The key typed for it.
    pub key: String,
    /// The name typed for a new account.
    pub account: String,
    /// The accounts there are.
    pub logins: Vec<Login>,
    /// The account in use for each agent, as agent and account id.
    pub in_use: Vec<(String, String)>,
    /// What the registry is searched for.
    pub query: String,
    /// What the registry offered for the last search.
    pub listings: Vec<ToolListing>,
    /// Whether a registry search is under way.
    pub searching: bool,
    /// The tool server form, while open.
    pub mcp: Option<McpForm>,
    /// The model server form, while open.
    pub model: Option<ModelForm>,
    /// The slot being edited, with its model and backups as typed.
    pub slot: Option<(String, String, String)>,
    /// The draft of a new customization, while open.
    pub draft: Option<Draft>,
    /// What the last form sent came to, such as a key saved or why it failed.
    pub outcome: Option<String>,
}

impl SettingsModel {
    /// Takes in `change` to a form and returns the requests it needs.
    pub(crate) fn take_form(&mut self, change: FormChange) -> Vec<Request> {
        let forms = &mut self.forms;
        forms.outcome = None;
        match change {
            FormChange::OpenAgent(agent) => {
                forms.agent = agent;
                forms.key.clear();
                forms.account.clear();
                return vec![Request::Accounts];
            }
            FormChange::KeyText(text) => forms.key = text,
            FormChange::SaveKey => {
                let key = forms.key.trim().to_owned();
                if let (Some(agent), false) = (forms.agent.clone(), key.is_empty()) {
                    forms.key.clear();
                    return vec![Request::SaveKey {
                        agent,
                        key: Some(key),
                        login: None,
                    }];
                }
            }
            FormChange::ForgetKey => {
                if let Some(agent) = forms.agent.clone() {
                    return vec![Request::SaveKey {
                        agent,
                        key: None,
                        login: None,
                    }];
                }
            }
            FormChange::SignIn(login) => {
                if let Some(agent) = forms.agent.clone() {
                    return vec![Request::SignIn { agent, login }];
                }
            }
            FormChange::AccountName(text) => forms.account = text,
            FormChange::AddAccount => {
                let name = forms.account.trim().to_owned();
                if let (Some(agent), false) = (forms.agent.clone(), name.is_empty()) {
                    forms.account.clear();
                    let id = account_id(&agent, &name, &forms.logins);
                    forms.logins.push(Login { agent, id, name });
                    return vec![self.save_accounts()];
                }
            }
            FormChange::UseAccount(id) => {
                if let Some(agent) = forms.agent.clone() {
                    forms.in_use.retain(|(held, _)| *held != agent);
                    forms.in_use.push((agent, id));
                    return vec![self.save_accounts()];
                }
            }
            FormChange::RemoveAccount(id) => {
                forms.logins.retain(|login| login.id != id);
                forms.in_use.retain(|(_, held)| *held != id);
                return vec![self.save_accounts()];
            }
            FormChange::RegistryQuery(text) => forms.query = text,
            FormChange::SearchRegistry => {
                forms.searching = true;
                return vec![Request::SearchTools {
                    query: forms.query.trim().to_owned(),
                }];
            }
            FormChange::InstallListing(place) => {
                if let Some(listing) = forms.listings.get(place).cloned() {
                    self.mcp_servers.push(McpServer {
                        name: listing.name,
                        target: listing.target,
                        env: listing.env,
                        description: listing.description,
                        website: listing.website,
                        enabled: true,
                        extension: None,
                    });
                    return vec![self.save_mcp_servers()];
                }
            }
            FormChange::ToggleMcpForm => {
                forms.mcp = match forms.mcp {
                    Some(_) => None,
                    None => Some(McpForm::default()),
                }
            }
            FormChange::McpField(field, text) => {
                let form = forms.mcp.get_or_insert_with(McpForm::default);
                match field {
                    McpField::Name => form.name = text,
                    McpField::Target => form.target = text,
                    McpField::Env => form.env = text,
                }
            }
            FormChange::AddMcp => {
                let Some(form) = forms.mcp.clone() else {
                    return Vec::new();
                };
                if form.name.trim().is_empty() || form.target.trim().is_empty() {
                    forms.outcome =
                        Some("A tool server needs a name and a command or address.".to_owned());
                    return Vec::new();
                }
                forms.mcp = None;
                self.mcp_servers.push(McpServer {
                    name: form.name.trim().to_owned(),
                    target: form.target.trim().to_owned(),
                    env: pairs(&form.env),
                    description: String::new(),
                    website: String::new(),
                    enabled: true,
                    extension: None,
                });
                return vec![self.save_mcp_servers()];
            }
            FormChange::ToggleModelForm => {
                forms.model = match forms.model {
                    Some(_) => None,
                    None => Some(ModelForm::default()),
                }
            }
            FormChange::ModelField(field, text) => {
                let form = forms.model.get_or_insert_with(ModelForm::default);
                match field {
                    ModelField::Id => form.id = text,
                    ModelField::Label => form.label = text,
                    ModelField::BaseUrl => form.base_url = text,
                    ModelField::Key => form.key = text,
                    ModelField::DefaultModel => form.default_model = text,
                }
            }
            FormChange::FindModels => {
                if let Some(form) = &mut forms.model
                    && !form.base_url.trim().is_empty()
                {
                    form.finding = true;
                    return vec![Request::FindModels {
                        server: model_server(form),
                    }];
                }
            }
            FormChange::SaveModelServer => {
                let Some(form) = forms.model.clone() else {
                    return Vec::new();
                };
                if form.id.trim().is_empty() || form.base_url.trim().is_empty() {
                    forms.outcome = Some("A model server needs an id and an address.".to_owned());
                    return Vec::new();
                }
                forms.model = None;
                return vec![Request::SaveModelServer {
                    server: model_server(&form),
                }];
            }
            FormChange::EditSlot(name) => {
                forms.slot = name.and_then(|name| {
                    let slot = self.slots.iter().find(|slot| slot.name == name)?;
                    Some((
                        name,
                        slot.model.clone().unwrap_or_default(),
                        slot.backups.join(", "),
                    ))
                });
            }
            FormChange::SlotModel(text) => {
                if let Some((_, model, _)) = &mut forms.slot {
                    *model = text;
                }
            }
            FormChange::SlotBackups(text) => {
                if let Some((_, _, backups)) = &mut forms.slot {
                    *backups = text;
                }
            }
            FormChange::SetSlot => {
                if let Some((name, model, backups)) = forms.slot.take() {
                    let model = model.trim();
                    return vec![slot_request(pm_harness::SlotChange::Set {
                        name,
                        scope: pm_harness::SlotScope::User,
                        model: (!model.is_empty()).then(|| model.to_owned()),
                        backups: list(&backups),
                        provider: None,
                    })];
                }
            }
            FormChange::ClearSlot(name) => {
                forms.slot = None;
                return vec![slot_request(pm_harness::SlotChange::Clear {
                    name,
                    scope: pm_harness::SlotScope::User,
                    provider: None,
                })];
            }
            FormChange::ApplyPreset(name) => {
                return vec![slot_request(pm_harness::SlotChange::ApplyPreset {
                    name,
                    scope: pm_harness::SlotScope::User,
                })];
            }
            FormChange::OpenDraft(kind) => {
                forms.draft = kind.map(|kind| Draft {
                    kind,
                    name: String::new(),
                    description: String::new(),
                    personal: false,
                });
            }
            FormChange::DraftName(text) => {
                if let Some(draft) = &mut forms.draft {
                    draft.name = text;
                }
            }
            FormChange::DraftDescription(text) => {
                if let Some(draft) = &mut forms.draft {
                    draft.description = text;
                }
            }
            FormChange::DraftPersonal(personal) => {
                if let Some(draft) = &mut forms.draft {
                    draft.personal = personal;
                }
            }
            FormChange::CreateDraft => return self.create_draft(),
        }
        Vec::new()
    }

    /// The request making the customization the draft describes, once its name reads and a project is open.
    fn create_draft(&mut self) -> Vec<Request> {
        let Some(draft) = self.forms.draft.clone() else {
            return Vec::new();
        };
        if !name_reads(&draft.name) {
            self.forms.outcome =
                Some("A name is lowercase letters, digits and hyphens.".to_owned());
            return Vec::new();
        }
        let Some((_, directory)) = self.library_place.clone() else {
            self.forms.outcome =
                Some("Add a project first; customizations are made in one.".to_owned());
            return Vec::new();
        };
        self.forms.draft = None;
        vec![Request::CreateCustomization {
            kind: draft.kind,
            scope: if draft.personal {
                Scope::User
            } else {
                Scope::Workspace
            },
            name: draft.name,
            description: draft.description,
            directory,
        }]
    }

    /// The request that keeps the accounts as they stand.
    fn save_accounts(&self) -> Request {
        Request::SetAccounts {
            logins: self.forms.logins.clone(),
            in_use: self.forms.in_use.clone(),
        }
    }

    /// The request that keeps the tool servers as they stand, those extensions bring left out.
    fn save_mcp_servers(&self) -> Request {
        Request::SaveMcpServers {
            servers: self
                .mcp_servers
                .iter()
                .filter(|server| server.extension.is_none())
                .cloned()
                .collect(),
        }
    }

    /// Takes in what the server answered to a form, giving back the requests that follow from it.
    pub fn take_form_answer(&mut self, response: Response) -> Vec<Request> {
        let forms = &mut self.forms;
        match response {
            Response::Accounts { logins, in_use } => {
                forms.logins = logins;
                forms.in_use = in_use;
            }
            Response::ToolListings { listings } => {
                forms.searching = false;
                forms.listings = listings;
            }
            Response::FoundModels { models, error } => {
                if let Some(form) = &mut forms.model {
                    form.finding = false;
                    if form.default_model.is_empty() {
                        form.default_model = models.first().cloned().unwrap_or_default();
                    }
                    form.models = models;
                }
                forms.outcome = error;
            }
            Response::Created { path } => {
                forms.outcome = Some(format!("Made {path}"));
                return self.asks_for(self.page.section);
            }
            Response::Done => forms.outcome = Some("Saved".to_owned()),
            Response::Failed { message } => {
                forms.searching = false;
                if let Some(form) = &mut forms.model {
                    form.finding = false;
                }
                forms.outcome = Some(message);
            }
            _ => {}
        }
        Vec::new()
    }
}

/// The request making `change` to the model slots.
fn slot_request(change: pm_harness::SlotChange) -> Request {
    Request::ModelSlots {
        session: None,
        change: Some(change),
    }
}

/// The model server `form` describes.
fn model_server(form: &ModelForm) -> pm_harness::ModelServer {
    let key = form.key.trim();
    let label = form.label.trim();
    let default_model = form.default_model.trim();
    pm_harness::ModelServer {
        id: form.id.trim().to_owned(),
        label: (!label.is_empty()).then(|| label.to_owned()),
        base_url: form.base_url.trim().to_owned(),
        wire: Default::default(),
        api_key_env: Vec::new(),
        api_key: (!key.is_empty()).then(|| key.to_owned()),
        has_api_key: false,
        headers: Vec::new(),
        default_model: (!default_model.is_empty()).then(|| default_model.to_owned()),
        models: form.models.clone(),
        replay_reasoning: false,
    }
}

/// The parts of `text` separated by commas, trimmed, the blank ones left out.
fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The `NAME=value` pairs of `text`, separated by commas.
fn pairs(text: &str) -> Vec<(String, String)> {
    list(text)
        .into_iter()
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            Some((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// Whether `name` reads as a customization's name: lowercase letters, digits and hyphens.
fn name_reads(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

/// An id for a new account of `agent` named `name` that none of `logins` holds.
fn account_id(agent: &str, name: &str, logins: &[Login]) -> String {
    let base: String = name
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    let base = format!("{agent}-{base}");
    let mut id = base.clone();
    let mut count = 2;
    while logins.iter().any(|login| login.id == id) {
        id = format!("{base}-{count}");
        count += 1;
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Variables are read as pairs and slots' backups as a list.
    #[test]
    fn reads_pairs_and_lists() {
        assert_eq!(
            pairs("A=1, B = two,broken"),
            vec![
                ("A".to_owned(), "1".to_owned()),
                ("B".to_owned(), "two".to_owned())
            ]
        );
        assert_eq!(list(" a, ,b "), vec!["a".to_owned(), "b".to_owned()]);
    }

    /// A tool server added from the form is saved with the others.
    #[test]
    fn adds_a_tool_server() {
        let mut model = SettingsModel::default();
        model.take_form(FormChange::ToggleMcpForm);
        model.take_form(FormChange::McpField(McpField::Name, "files".to_owned()));
        model.take_form(FormChange::McpField(
            McpField::Target,
            "npx files".to_owned(),
        ));
        let requests = model.take_form(FormChange::AddMcp);
        assert!(
            matches!(&requests[..], [Request::SaveMcpServers { servers }] if servers.len() == 1)
        );
        assert!(model.forms.mcp.is_none());
    }

    /// A draft needs a name that reads and a project to be made in.
    #[test]
    fn checks_drafts() {
        let mut model = SettingsModel::default();
        model.take_form(FormChange::OpenDraft(Some(Kind::Skill)));
        model.take_form(FormChange::DraftName("Bad Name".to_owned()));
        assert!(model.take_form(FormChange::CreateDraft).is_empty());
        model.take_form(FormChange::DraftName("good-name".to_owned()));
        assert!(model.take_form(FormChange::CreateDraft).is_empty());
        model.library_place = Some(("claude".to_owned(), "/p".to_owned()));
        let requests = model.take_form(FormChange::CreateDraft);
        assert!(matches!(
            &requests[..],
            [Request::CreateCustomization { .. }]
        ));
    }

    /// Accounts get ids of their own and the one in use is kept per agent.
    #[test]
    fn keeps_accounts() {
        let mut model = SettingsModel::default();
        model.take_form(FormChange::OpenAgent(Some("claude".to_owned())));
        model.take_form(FormChange::AccountName("Work".to_owned()));
        model.take_form(FormChange::AddAccount);
        model.take_form(FormChange::AccountName("Work".to_owned()));
        model.take_form(FormChange::AddAccount);
        let ids: Vec<&str> = model
            .forms
            .logins
            .iter()
            .map(|login| login.id.as_str())
            .collect();
        assert_eq!(ids, vec!["claude-work", "claude-work-2"]);
        model.take_form(FormChange::UseAccount("claude-work".to_owned()));
        assert_eq!(
            model.forms.in_use,
            vec![("claude".to_owned(), "claude-work".to_owned())]
        );
    }
}
