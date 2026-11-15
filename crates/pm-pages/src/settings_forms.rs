//! The forms inside the settings window's Agents sections: an agent's key,
//! sign-in and accounts; the tool server registry and the form adding a tool
//! server; the form declaring a model server; the slot being edited; and the
//! draft of a new customization.

use rustle::prelude::*;

use pm_components::{
    BadgeVariant, Button, ButtonVariant, CardRow, Label, LabelVariant, Notice, NoticeVariant,
    SectionHeader, SectionHeaderVariant, Size, Switch, TextInput,
};
use pm_shell::{
    Draft, FormChange, McpField, McpForm, ModelField, ModelForm, SettingsChange, SettingsModel,
};

use crate::settings_actions::SettingsActions;
use crate::settings_body::{boxed, divided, heading, note};

/// A callback that sends the form change `make` builds from the text it is given.
fn typed(
    actions: &SettingsActions,
    make: impl Fn(String) -> FormChange + 'static,
) -> impl Fn(String) + 'static {
    actions.with(move |text| SettingsChange::Form(make(text)))
}

/// A callback that sends the form change `change`.
fn pressed(actions: &SettingsActions, change: FormChange) -> impl Fn() + 'static {
    actions.send(SettingsChange::Form(change))
}

/// A labelled field holding `value`, sending what is typed through `on_change`.
fn field(label: &str, value: &str, placeholder: &str, on_change: impl Fn(String) + 'static) {
    Label::new(label)
        .variant(LabelVariant::Muted)
        .size(Size::Xs)
        .show();
    spacer(Modifier::new().height(pm_components::spacing(4.0)));
    TextInput::new(value)
        .placeholder(placeholder)
        .size(Size::Sm)
        .modifier(Modifier::new().fill_max_width())
        .on_change(on_change)
        .show();
    spacer(Modifier::new().height(pm_components::spacing(10.0)));
}

/// A row of buttons at the end of a form.
fn buttons(content: impl FnOnce()) {
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::End,
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(pm_components::spacing(8.0)),
                Alignment::Center,
                content,
            )
        },
    );
}

/// A padded column inside a box.
fn form(content: impl FnOnce()) {
    boxed(|| column_with(Modifier::new().fill_max_width().padding(12.0), content));
}

/// What the last form sent came to, when it said.
pub(crate) fn outcome(model: &SettingsModel) {
    if let Some(outcome) = &model.forms.outcome {
        let failed = outcome != "Saved" && !outcome.starts_with("Made ");
        Notice::new(outcome.clone())
            .variant(if failed {
                NoticeVariant::Danger
            } else {
                NoticeVariant::Info
            })
            .size(Size::Sm)
            .show();
    }
}

/// The key, sign-in and accounts of the agent whose row was opened.
pub(crate) fn agent_form(model: &SettingsModel, actions: &SettingsActions, agent: &str) {
    let forms = &model.forms;
    form(|| {
        Label::new("Key").size(Size::Md).show();
        spacer(Modifier::new().height(pm_components::spacing(4.0)));
        note("A key is kept on the server's machine and given to the agent in place of a login.");
        spacer(Modifier::new().height(pm_components::spacing(8.0)));
        TextInput::new(forms.key.clone())
            .placeholder("Paste a key")
            .size(Size::Sm)
            .modifier(Modifier::new().fill_max_width())
            .on_change(typed(actions, FormChange::KeyText))
            .on_submit({
                let save = pressed(actions, FormChange::SaveKey);
                move |_| save()
            })
            .show();
        spacer(Modifier::new().height(pm_components::spacing(8.0)));
        buttons(|| {
            Button::new("Forget key")
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::ForgetKey))
                .show();
            Button::new("Save key")
                .variant(ButtonVariant::Primary)
                .size(Size::Sm)
                .disabled(forms.key.trim().is_empty())
                .on_click(pressed(actions, FormChange::SaveKey))
                .show();
        });
        spacer(Modifier::new().height(pm_components::spacing(14.0)));
        Label::new("Sign in").size(Size::Md).show();
        spacer(Modifier::new().height(pm_components::spacing(4.0)));
        note(
            "Signs in with the agent's own login; a code to enter in the browser shows in the main window.",
        );
        spacer(Modifier::new().height(pm_components::spacing(8.0)));
        buttons(|| {
            Button::new("Sign in")
                .icon(phosphor::SIGN_IN)
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::SignIn(None)))
                .show();
        });
        spacer(Modifier::new().height(pm_components::spacing(14.0)));
        let logins: Vec<_> = forms
            .logins
            .iter()
            .filter(|login| login.agent == agent)
            .cloned()
            .collect();
        heading("Accounts", logins.len());
        if logins.is_empty() {
            note("No added account. Add one to switch between logins.");
        } else {
            boxed(|| {
                divided(logins, |_, login| {
                    let used = forms
                        .in_use
                        .iter()
                        .any(|(held, id)| held == agent && *id == login.id);
                    let mut row = CardRow::new(login.name.clone())
                        .icon(phosphor::USER_CIRCLE)
                        .detail(login.id.clone());
                    if used {
                        row = row.badge("In use", BadgeVariant::Accent);
                    }
                    let sign_in = pressed(actions, FormChange::SignIn(Some(login.id.clone())));
                    let pick = pressed(actions, FormChange::UseAccount(login.id.clone()));
                    let remove = pressed(actions, FormChange::RemoveAccount(login.id.clone()));
                    row.trailing(move || {
                        Button::new("Sign in")
                            .size(Size::Xs)
                            .on_click(sign_in)
                            .show();
                        if !used {
                            Button::new("Use").size(Size::Xs).on_click(pick).show();
                        }
                        Button::new("Remove").size(Size::Xs).on_click(remove).show();
                    })
                    .show();
                });
            });
        }
        spacer(Modifier::new().height(pm_components::spacing(8.0)));
        row_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::SpacedBy(pm_components::spacing(8.0)),
            Alignment::Center,
            || {
                TextInput::new(forms.account.clone())
                    .placeholder("Account name, such as Work")
                    .size(Size::Sm)
                    .modifier(Modifier::new().weight(1.0))
                    .on_change(typed(actions, FormChange::AccountName))
                    .show();
                Button::new("Add account")
                    .icon(phosphor::PLUS)
                    .size(Size::Sm)
                    .variant(ButtonVariant::Secondary)
                    .disabled(forms.account.trim().is_empty())
                    .on_click(pressed(actions, FormChange::AddAccount))
                    .show();
            },
        );
    });
}

/// The registry's search box and what it offered, each listing with the button installing it.
pub(crate) fn registry(model: &SettingsModel, actions: &SettingsActions) {
    let forms = &model.forms;
    SectionHeader::new("Add a server")
        .variant(SectionHeaderVariant::Group)
        .show();
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        || {
            TextInput::new(forms.query.clone())
                .placeholder("Find servers in the public MCP registry")
                .modifier(Modifier::new().weight(1.0))
                .on_change(typed(actions, FormChange::RegistryQuery))
                .on_submit({
                    let search = pressed(actions, FormChange::SearchRegistry);
                    move |_| search()
                })
                .show();
            Button::new(if forms.searching {
                "Searching…"
            } else {
                "Search"
            })
            .size(Size::Sm)
            .variant(ButtonVariant::Secondary)
            .disabled(forms.searching)
            .on_click(pressed(actions, FormChange::SearchRegistry))
            .show();
            Button::new("Add by hand")
                .icon(phosphor::PLUS)
                .size(Size::Sm)
                .variant(ButtonVariant::Secondary)
                .on_click(pressed(actions, FormChange::ToggleMcpForm))
                .show();
        },
    );
    if let Some(mcp) = &forms.mcp {
        mcp_form(mcp, actions);
    }
    if forms.listings.is_empty() {
        return;
    }
    boxed(|| {
        divided(forms.listings.clone(), |place, listing| {
            let installed = model
                .mcp_servers
                .iter()
                .any(|server| server.name == listing.name);
            let mut row = CardRow::new(listing.title.clone())
                .icon(phosphor::PLUGS)
                .detail(listing.description.clone());
            if !listing.inputs.is_empty() {
                row = row.badge(
                    format!("{} to fill in", listing.inputs.len()),
                    BadgeVariant::Warning,
                );
            }
            let install = pressed(actions, FormChange::InstallListing(place));
            row.trailing(move || {
                Button::new(if installed { "Installed" } else { "Install" })
                    .size(Size::Xs)
                    .variant(ButtonVariant::Primary)
                    .disabled(installed)
                    .on_click(install)
                    .show();
            })
            .show();
        });
    });
}

/// The form adding a tool server by hand.
fn mcp_form(mcp: &McpForm, actions: &SettingsActions) {
    form(|| {
        Label::new("New MCP server").size(Size::Md).show();
        spacer(Modifier::new().height(pm_components::spacing(10.0)));
        field(
            "Name",
            &mcp.name,
            "files",
            typed(actions, |text| FormChange::McpField(McpField::Name, text)),
        );
        field(
            "Command or address",
            &mcp.target,
            "npx -y @modelcontextprotocol/server-filesystem ~ or https://…",
            typed(actions, |text| FormChange::McpField(McpField::Target, text)),
        );
        field(
            "Variables or headers",
            &mcp.env,
            "NAME=value, OTHER=value",
            typed(actions, |text| FormChange::McpField(McpField::Env, text)),
        );
        buttons(|| {
            Button::new("Cancel")
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::ToggleMcpForm))
                .show();
            Button::new("Add server")
                .variant(ButtonVariant::Primary)
                .size(Size::Sm)
                .disabled(mcp.name.trim().is_empty() || mcp.target.trim().is_empty())
                .on_click(pressed(actions, FormChange::AddMcp))
                .show();
        });
    });
}

/// The button opening the form that declares a model server, and the form while open.
pub(crate) fn model_server_form(model: &SettingsModel, actions: &SettingsActions) {
    let Some(form_state) = &model.forms.model else {
        buttons(|| {
            Button::new("Declare a model server")
                .icon(phosphor::PLUS)
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::ToggleModelForm))
                .show();
        });
        return;
    };
    model_form(form_state, actions);
}

/// The form declaring a model server.
fn model_form(state: &ModelForm, actions: &SettingsActions) {
    form(|| {
        Label::new("New model server").size(Size::Md).show();
        spacer(Modifier::new().height(pm_components::spacing(4.0)));
        note(
            "Any OpenAI-compatible server: LiteLLM, vLLM, LM Studio, llama.cpp or a hosted gateway.",
        );
        spacer(Modifier::new().height(pm_components::spacing(10.0)));
        field(
            "Id",
            &state.id,
            "local-vllm",
            typed(actions, |text| FormChange::ModelField(ModelField::Id, text)),
        );
        field(
            "Name",
            &state.label,
            "Local vLLM",
            typed(actions, |text| {
                FormChange::ModelField(ModelField::Label, text)
            }),
        );
        field(
            "Address",
            &state.base_url,
            "http://localhost:8000/v1",
            typed(actions, |text| {
                FormChange::ModelField(ModelField::BaseUrl, text)
            }),
        );
        field(
            "Key",
            &state.key,
            "Leave empty when it needs none",
            typed(actions, |text| {
                FormChange::ModelField(ModelField::Key, text)
            }),
        );
        field(
            "First model",
            &state.default_model,
            "Found by asking the server",
            typed(actions, |text| {
                FormChange::ModelField(ModelField::DefaultModel, text)
            }),
        );
        if !state.models.is_empty() {
            note(&format!("Offers {}", state.models.join(", ")));
            spacer(Modifier::new().height(pm_components::spacing(10.0)));
        }
        buttons(|| {
            Button::new("Cancel")
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::ToggleModelForm))
                .show();
            Button::new(if state.finding {
                "Asking…"
            } else {
                "Find models"
            })
            .variant(ButtonVariant::Secondary)
            .size(Size::Sm)
            .disabled(state.finding || state.base_url.trim().is_empty())
            .on_click(pressed(actions, FormChange::FindModels))
            .show();
            Button::new("Save server")
                .variant(ButtonVariant::Primary)
                .size(Size::Sm)
                .disabled(state.id.trim().is_empty() || state.base_url.trim().is_empty())
                .on_click(pressed(actions, FormChange::SaveModelServer))
                .show();
        });
    });
}

/// The form editing the slot `name`, with its model and backups as typed.
pub(crate) fn slot_form(
    (name, model_text, backups): (&str, &str, &str),
    actions: &SettingsActions,
) {
    form(|| {
        Label::new(format!("@{name}")).size(Size::Md).show();
        spacer(Modifier::new().height(pm_components::spacing(10.0)));
        field(
            "Model",
            model_text,
            "provider/model:effort, or empty for the conversation's",
            typed(actions, FormChange::SlotModel),
        );
        field(
            "Backups",
            backups,
            "Tried in order when the model fails, separated by commas",
            typed(actions, FormChange::SlotBackups),
        );
        let name = name.to_owned();
        buttons(|| {
            Button::new("Inherit")
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::ClearSlot(name)))
                .show();
            Button::new("Cancel")
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::EditSlot(None)))
                .show();
            Button::new("Set slot")
                .variant(ButtonVariant::Primary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::SetSlot))
                .show();
        });
    });
}

/// The draft of a new customization of the kind the page lists, or the button opening it.
pub(crate) fn draft_form(
    model: &SettingsModel,
    actions: &SettingsActions,
    kind: pm_shell::Kind,
    noun: &str,
) {
    let Some(draft) = model
        .forms
        .draft
        .as_ref()
        .filter(|draft| draft.kind == kind)
    else {
        buttons(|| {
            Button::new(format!("New {noun}"))
                .icon(phosphor::PLUS)
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::OpenDraft(Some(kind))))
                .show();
        });
        return;
    };
    draft_fields(draft, actions, noun);
}

/// The fields of the draft and the buttons making it.
fn draft_fields(draft: &Draft, actions: &SettingsActions, noun: &str) {
    form(|| {
        Label::new(format!("New {noun}")).size(Size::Md).show();
        spacer(Modifier::new().height(pm_components::spacing(10.0)));
        field(
            "Name",
            &draft.name,
            "lowercase-with-hyphens",
            typed(actions, FormChange::DraftName),
        );
        field(
            "What it is for",
            &draft.description,
            "One line the agent reads to decide when to use it",
            typed(actions, FormChange::DraftDescription),
        );
        let personal = draft.personal;
        row_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::SpacedBy(pm_components::spacing(8.0)),
            Alignment::Center,
            || {
                Switch::new(personal)
                    .on_change({
                        let change = actions.change.clone();
                        move |on| change(SettingsChange::Form(FormChange::DraftPersonal(on)))
                    })
                    .show();
                Label::new("Keep it in my own folder, for every project")
                    .variant(LabelVariant::Muted)
                    .size(Size::Sm)
                    .show();
            },
        );
        spacer(Modifier::new().height(pm_components::spacing(12.0)));
        buttons(|| {
            Button::new("Cancel")
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(pressed(actions, FormChange::OpenDraft(None)))
                .show();
            Button::new("Create")
                .variant(ButtonVariant::Primary)
                .size(Size::Sm)
                .disabled(draft.name.trim().is_empty())
                .on_click(pressed(actions, FormChange::CreateDraft))
                .show();
        });
    });
}
