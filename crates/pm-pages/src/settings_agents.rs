//! The sections inside Agents that list what the server has: the agents,
//! whether each is offered, and each one's key, sign-in and accounts; the
//! tool servers with the registry and the form adding one; the model servers
//! with the form declaring one; and the model slots, edited one at a time,
//! with their presets.

use rustle::prelude::*;

use pm_components::{
    BadgeVariant, Button, CardRow, CardRowVariant, IconButton, Label, LabelVariant, SectionHeader,
    SectionHeaderVariant, Size, Switch, TextInput, TextInputVariant,
};
use pm_shell::{FormChange, Section, SettingsChange, SettingsModel, slot_source};

use crate::settings_actions::SettingsActions;
use crate::settings_body::{boxed, divided, heading, note};

/// Emits what `section` lists.
pub(crate) fn body(model: &SettingsModel, actions: &SettingsActions, section: Section) {
    match section {
        Section::AgentServers => agents(model, actions),
        Section::McpServers => mcp_servers(model, actions),
        Section::ModelServers => model_servers(model, actions),
        _ => slots(model, actions),
    }
    crate::settings_forms::outcome(model);
}

/// Emits every agent the harness offers, each with the switch offering it for new sessions.
fn agents(model: &SettingsModel, actions: &SettingsActions) {
    heading("Agents", model.agents.len());
    boxed(|| {
        divided(model.agents.clone(), |_, agent| {
            let shown = model.agent_shown(&agent.id);
            let last = shown && model.shown_agents() <= 1;
            let detail = if agent.server {
                "A model server declared on the Model Servers page.".to_owned()
            } else {
                agent.hint.clone().unwrap_or_default()
            };
            let toggle = actions.send(SettingsChange::ToggleAgent(agent.id.clone()));
            let mut row = CardRow::new(agent.name.clone())
                .icon(phosphor::ROBOT)
                .detail(detail)
                .variant(if shown {
                    CardRowVariant::Plain
                } else {
                    CardRowVariant::Dimmed
                });
            if !shown {
                row = row.badge("Hidden", BadgeVariant::Neutral);
            }
            if model.machine.agents.default_agent.as_deref() == Some(agent.id.as_str()) {
                row = row.badge("Default", BadgeVariant::Accent);
            }
            let open = model.forms.agent.as_deref() == Some(agent.id.as_str());
            let manage = actions.send(SettingsChange::Form(FormChange::OpenAgent(
                (!open).then(|| agent.id.clone()),
            )));
            row.trailing(move || {
                Button::new(if open { "Done" } else { "Sign in & keys" })
                    .size(Size::Xs)
                    .on_click(manage)
                    .show();
                Switch::new(shown)
                    .disabled(last)
                    .on_change(move |_| toggle())
                    .show();
            })
            .show();
            if open {
                column_with(Modifier::new().fill_max_width().padding(8.0), || {
                    crate::settings_forms::agent_form(model, actions, &agent.id)
                });
            }
        });
    });
}

/// Emits the tool servers, narrowed by the search box, each with its switch and the button taking it out.
fn mcp_servers(model: &SettingsModel, actions: &SettingsActions) {
    TextInput::new(model.page.item_find.clone())
        .variant(TextInputVariant::Search)
        .placeholder("Search MCP servers")
        .modifier(Modifier::new().fill_max_width())
        .on_change(actions.with(SettingsChange::ItemFind))
        .show();
    crate::settings_forms::registry(model, actions);
    if !model.mcp_loaded {
        note("Asking the server which MCP servers there are…");
        return;
    }
    let needle = model.page.item_find.trim().to_lowercase();
    let listed: Vec<usize> = model
        .mcp_servers
        .iter()
        .enumerate()
        .filter(|(_, server)| {
            needle.is_empty()
                || server.name.to_lowercase().contains(&needle)
                || server.description.to_lowercase().contains(&needle)
        })
        .map(|(place, _)| place)
        .collect();
    heading("Installed", listed.len());
    if listed.is_empty() {
        note("No MCP server is installed. Agents are given only their own tools.");
        return;
    }
    boxed(|| {
        divided(listed, |_, place| {
            let server = &model.mcp_servers[place];
            let detail = if server.description.is_empty() {
                server.target.clone()
            } else {
                format!("{} · {}", server.description, server.target)
            };
            let mut row = CardRow::new(server.name.clone())
                .icon(phosphor::PLUGS)
                .detail(detail)
                .variant(if server.enabled {
                    CardRowVariant::Plain
                } else {
                    CardRowVariant::Dimmed
                });
            if !server.env.is_empty() {
                row = row.badge(
                    format!("{} variables", server.env.len()),
                    BadgeVariant::Neutral,
                );
            }
            let from_extension = server.extension.is_some();
            if from_extension {
                row = row.badge("Extension", BadgeVariant::Accent);
            }
            let enabled = server.enabled;
            let toggle = actions.send(SettingsChange::ToggleMcp(place));
            let remove = actions.send(SettingsChange::RemoveMcp(place));
            row.trailing(move || {
                Switch::new(enabled).on_change(move |_| toggle()).show();
                if !from_extension {
                    IconButton::new(phosphor::TRASH)
                        .size(Size::Xs)
                        .on_click(remove)
                        .show();
                }
            })
            .show();
        });
    });
}

/// Emits the model servers the reader declared, each with the button taking it off the machine.
fn model_servers(model: &SettingsModel, actions: &SettingsActions) {
    if !model.model_servers_loaded {
        note("Asking the server which model servers are declared…");
        return;
    }
    crate::settings_forms::model_server_form(model, actions);
    heading("Model servers", model.model_servers.len());
    if model.model_servers.is_empty() {
        note("No model server is declared.");
        return;
    }
    boxed(|| {
        divided(model.model_servers.clone(), |_, server| {
            let models = match server.models.len() {
                0 => "models it lists".to_owned(),
                1 => "1 model".to_owned(),
                count => format!("{count} models"),
            };
            let mut row = CardRow::new(server.label.clone().unwrap_or_else(|| server.id.clone()))
                .icon(phosphor::LIGHTNING)
                .detail(format!("{} · {models}", server.base_url));
            if server.has_api_key {
                row = row.badge("Key saved", BadgeVariant::Success);
            } else if !server.api_key_env.is_empty() {
                row = row.badge(server.api_key_env.join(", "), BadgeVariant::Neutral);
            }
            let remove = actions.send(SettingsChange::RemoveModelServer(server.id.clone()));
            row.trailing(move || {
                IconButton::new(phosphor::TRASH)
                    .size(Size::Xs)
                    .on_click(remove)
                    .show();
            })
            .show();
        });
    });
}

/// Emits every model slot with the model it resolves to and its backups, then the presets.
fn slots(model: &SettingsModel, actions: &SettingsActions) {
    if !model.slots_loaded {
        note("Asking the agent which model slots there are…");
        return;
    }
    heading("Slots", model.slots.len());
    boxed(|| {
        divided(model.slots.clone(), |_, slot| {
            let model_name = slot
                .model
                .clone()
                .or_else(|| slot.resolved.clone())
                .unwrap_or_else(|| "The conversation's model".to_owned());
            let detail = if slot.backups.is_empty() {
                model_name
            } else {
                format!("{model_name} · then {}", slot.backups.join(", "))
            };
            let mut row = CardRow::new(format!("@{}", slot.name))
                .code()
                .detail(detail)
                .badge(slot_source(&slot), BadgeVariant::Neutral);
            if let (true, Some(suggested)) = (slot.inherited, &slot.suggested) {
                row = row.badge(format!("Suggested: {suggested}"), BadgeVariant::Accent);
            }
            let editing = model
                .forms
                .slot
                .as_ref()
                .filter(|(name, _, _)| *name == slot.name);
            let edit = actions.send(SettingsChange::Form(FormChange::EditSlot(
                editing.is_none().then(|| slot.name.clone()),
            )));
            let label = if editing.is_some() { "Close" } else { "Edit" };
            row.trailing(move || Button::new(label).size(Size::Xs).on_click(edit).show())
                .show();
            if let Some((name, chosen, backups)) = editing {
                column_with(Modifier::new().fill_max_width().padding(8.0), || {
                    crate::settings_forms::slot_form((name, chosen, backups), actions)
                });
            }
        });
    });
    SectionHeader::new(format!("Presets · {}", model.presets.len()))
        .variant(SectionHeaderVariant::Group)
        .show();
    if model.presets.is_empty() {
        Label::new("No preset is defined.")
            .variant(LabelVariant::Subtle)
            .show();
        return;
    }
    boxed(|| {
        divided(model.presets.clone(), |_, preset| {
            let set: Vec<String> = preset
                .slots
                .iter()
                .map(|slot| match &slot.model {
                    Some(chosen) => format!("@{} {chosen}", slot.name),
                    None => format!("@{}", slot.name),
                })
                .collect();
            let mut row = CardRow::new(preset.name.clone()).detail(set.join(" · "));
            if let Some(effort) = &preset.effort {
                row = row.badge(effort.clone(), BadgeVariant::Neutral);
            }
            let apply = actions.send(SettingsChange::Form(FormChange::ApplyPreset(
                preset.name.clone(),
            )));
            row.trailing(move || Button::new("Apply").size(Size::Xs).on_click(apply).show())
                .show();
        });
    });
}
