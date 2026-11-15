//! The pages of what agents are given in the project: the overview, each
//! kind of customization with its switch and the draft making a new one,
//! and the tools in their sets.

use rustle::prelude::*;

use pm_components::{
    BadgeVariant, CardRow, CardRowVariant, SectionHeader, SectionHeaderVariant, Switch, TextInput,
    TextInputVariant,
};
use pm_shell::{Section, SettingsChange, SettingsModel};

use crate::settings_actions::SettingsActions;
use crate::settings_body::{boxed, divided, heading, note};

/// The pages the overview leads to.
const PAGES: [Section; 5] = [
    Section::Skills,
    Section::Instructions,
    Section::AgentDefinitions,
    Section::Hooks,
    Section::Tools,
];

/// Emits what `section` lists.
pub(crate) fn body(model: &SettingsModel, actions: &SettingsActions, section: Section) {
    match &model.library_place {
        Some((agent, directory)) => note(&format!("What {agent} is given in {directory}.")),
        None => {
            note("Add a project to see what agents are given in it.");
            return;
        }
    }
    if !model.library_loaded {
        note("Asking the server what agents are given…");
        return;
    }
    match section {
        Section::Customizations => overview(model, actions),
        Section::Tools => tools(model, actions),
        section => items(model, actions, section),
    }
}

/// Emits a card for each page, with how many it lists, leading to it.
fn overview(model: &SettingsModel, actions: &SettingsActions) {
    boxed(|| {
        divided(PAGES.to_vec(), |_, page| {
            let count = model.library_count(page).unwrap_or_default();
            column_with(
                Modifier::new()
                    .fill_max_width()
                    .cursor(CursorIcon::Pointer)
                    .clickable(actions.send(SettingsChange::Open(page))),
                || {
                    CardRow::new(page.title())
                        .icon(crate::settings_nav::section_icon(page))
                        .detail(page.blurb())
                        .badge(count.to_string(), BadgeVariant::Neutral)
                        .show();
                },
            );
        });
    });
}

/// Emits the customizations of the kind `section` lists, narrowed by the search box, each with its switch.
fn items(model: &SettingsModel, actions: &SettingsActions, section: Section) {
    let Some(kind) = section.kind() else {
        return;
    };
    TextInput::new(model.page.item_find.clone())
        .variant(TextInputVariant::Search)
        .placeholder(format!("Search {}", section.title().to_lowercase()))
        .modifier(Modifier::new().fill_max_width())
        .on_change(actions.with(SettingsChange::ItemFind))
        .show();
    if let Some(noun) = drafted_noun(kind) {
        crate::settings_forms::draft_form(model, actions, kind, noun);
        crate::settings_forms::outcome(model);
    }
    let needle = model.page.item_find.trim().to_lowercase();
    let found: Vec<_> = model
        .items_of(kind)
        .into_iter()
        .filter(|item| {
            needle.is_empty()
                || item.name.to_lowercase().contains(&needle)
                || item.description.to_lowercase().contains(&needle)
        })
        .cloned()
        .collect();
    heading(section.title(), found.len());
    if found.is_empty() {
        note("None found.");
        return;
    }
    boxed(|| {
        divided(found, |_, item| {
            let key = item.key();
            let enabled = model.enabled(&key);
            let detail = match (item.description.is_empty(), item.command.is_empty()) {
                (false, _) => item.description.clone(),
                (true, false) => item.command.clone(),
                (true, true) => item.path.clone(),
            };
            let mut row = CardRow::new(item.name.clone())
                .detail(detail)
                .variant(if enabled {
                    CardRowVariant::Plain
                } else {
                    CardRowVariant::Dimmed
                });
            if !item.tool.is_empty() {
                row = row.badge(item.tool.clone(), BadgeVariant::Neutral);
            }
            if !item.model.is_empty() {
                row = row.badge(item.model.clone(), BadgeVariant::Accent);
            }
            let toggle = actions.send(SettingsChange::ToggleWithheld(key));
            row.trailing(move || Switch::new(enabled).on_change(move |_| toggle()).show())
                .show();
        });
    });
}

/// What a new customization of `kind` is called, for the kinds a draft can make.
fn drafted_noun(kind: pm_shell::Kind) -> Option<&'static str> {
    match kind {
        pm_shell::Kind::Skill => Some("skill"),
        pm_shell::Kind::Instruction => Some("instruction file"),
        pm_shell::Kind::Agent => Some("agent definition"),
        _ => None,
    }
}

/// Emits the sets of tools, each with its switch over its tools and theirs.
fn tools(model: &SettingsModel, actions: &SettingsActions) {
    if model.tool_sets.is_empty() {
        note("No tools were found.");
        return;
    }
    for (place, set) in model.tool_sets.iter().enumerate() {
        key(place, || {
            let set_key = set.key();
            let set_on = model.enabled(&set_key);
            let toggle = actions.send(SettingsChange::ToggleWithheld(set_key));
            SectionHeader::new(format!("{} · {}", set.name, set.tools.len()))
                .variant(SectionHeaderVariant::Group)
                .show();
            boxed(|| {
                let mut header = CardRow::new("Every tool of the set");
                if let Some(error) = &set.error {
                    header = header.detail(error.clone());
                }
                header
                    .trailing(move || Switch::new(set_on).on_change(move |_| toggle()).show())
                    .show();
                for tool in &set.tools {
                    key(tool.name.clone(), || {
                        pm_components::Divider::new().show();
                        let tool_key = set.tool_key(&tool.name);
                        let on = set_on && model.enabled(&tool_key);
                        let toggle = actions.send(SettingsChange::ToggleWithheld(tool_key));
                        CardRow::new(tool.name.clone())
                            .code()
                            .detail(tool.description.clone())
                            .variant(if on {
                                CardRowVariant::Plain
                            } else {
                                CardRowVariant::Dimmed
                            })
                            .trailing(move || {
                                Switch::new(on)
                                    .disabled(!set_on)
                                    .on_change(move |_| toggle())
                                    .show()
                            })
                            .show();
                    });
                }
            });
        });
    }
}
