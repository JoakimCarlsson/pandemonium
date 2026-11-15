//! The list of sections down the left side of the settings window, with the
//! box every setting is searched with over it.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{IconButton, Leading, Size, TextInput, TextInputVariant, Tile, TileVariant};
use pm_shell::{Section, SettingsChange, SettingsModel};

use crate::settings::Layout;
use crate::settings_actions::SettingsActions;

/// The glyph before the name of `section` in the list.
pub(crate) fn section_icon(section: Section) -> Icon {
    match section {
        Section::General | Section::Results => phosphor::GEAR_SIX,
        Section::Appearance => phosphor::PAINT_BRUSH,
        Section::Languages => phosphor::TRANSLATE,
        Section::Keymap => phosphor::KEYBOARD,
        Section::Terminal => phosphor::TERMINAL_WINDOW,
        Section::Browser => phosphor::GLOBE,
        Section::Git | Section::Worktrees => phosphor::GIT_BRANCH,
        Section::Servers => phosphor::HARD_DRIVES,
        Section::Editor
        | Section::EditorAppearance
        | Section::EditorEditing
        | Section::EditorSaving
        | Section::EditorIntelligence
        | Section::EditorSearch
        | Section::EditorVim => phosphor::FILE_CODE,
        Section::Agents
        | Section::ModelSlots
        | Section::Customizations
        | Section::Skills
        | Section::Instructions
        | Section::AgentDefinitions
        | Section::Hooks
        | Section::Tools => phosphor::SPARKLE,
        Section::AgentServers => phosphor::ROBOT,
        Section::McpServers => phosphor::PLUGS,
        Section::ModelServers => phosphor::LIGHTNING,
    }
}

/// Whether `section` shows in the list while `showing` is open: the sections inside Agents unless folded, and those inside another only while it or one of its sections shows.
fn listed(section: Section, showing: Section, folded: bool) -> bool {
    match section.parent() {
        Some(Section::Agents) => !folded,
        Some(parent) => showing == parent || showing.parent() == Some(parent),
        None => true,
    }
}

/// Emits the list of sections for `model`, `layout.navigation` wide.
pub(crate) fn navigation(model: &Rc<SettingsModel>, actions: &SettingsActions, layout: Layout) {
    let scheme = color_scheme();
    let scroll = remember_scroll_state();
    let picked = model.page.section;
    let searching = model.searching();
    let folded = model.page.folded;
    column_arranged(
        Modifier::new()
            .width(layout.navigation)
            .fill_max_height()
            .background(scheme.surface_container_low)
            .border_right_hairline(scheme.outline_variant)
            .padding(12.0),
        Arrangement::SpacedBy(8.0),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(4.0),
                Alignment::Center,
                || {
                    TextInput::new(model.page.find.clone())
                        .variant(TextInputVariant::Search)
                        .placeholder("Search settings")
                        .modifier(Modifier::new().weight(1.0))
                        .on_change(actions.with(SettingsChange::Find))
                        .show();
                    if searching {
                        IconButton::new(phosphor::X)
                            .size(Size::Xs)
                            .on_click(actions.send(SettingsChange::Find(String::new())))
                            .show();
                    }
                },
            );
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .vertical_scroll(&scroll),
                Arrangement::SpacedBy(1.0),
                Alignment::Start,
                || {
                    for section in Section::ALL {
                        if !listed(section, picked, folded) {
                            continue;
                        }
                        key(section, || {
                            entry(model, actions, section, !searching && section == picked)
                        });
                    }
                },
            );
        },
    );
}

/// One row of the list: a section of its own leads with its glyph, Agents with the chevron saying whether its sections show, and those inside another are set in under it, with their count after them when they have one.
fn entry(model: &SettingsModel, actions: &SettingsActions, section: Section, selected: bool) {
    let leading = match section {
        Section::Agents if model.page.folded => Leading::Icon(phosphor::CARET_RIGHT),
        Section::Agents => Leading::Icon(phosphor::CARET_DOWN),
        _ if section.parent().is_some() => Leading::None,
        _ => Leading::Icon(section_icon(section)),
    };
    let mut tile = Tile::new(section.title())
        .leading(leading)
        .variant(TileVariant::Plain)
        .indent(if section.parent().is_some() {
            26.0
        } else {
            0.0
        })
        .selected(selected)
        .on_click(actions.send(SettingsChange::Open(section)));
    if let Some(count) = model.library_count(section) {
        tile = tile.trailing(count.to_string());
    }
    tile.show();
}
