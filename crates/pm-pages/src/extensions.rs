//! The extensions page: what the server on this machine has installed, then
//! what it can install, narrowed by a search, each a row saying what the
//! extension is for and how it stands, with the button that installs,
//! updates or uninstalls it.

use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, FilterField, ItemRow, Label, LabelVariant, PageBody, PageHeader,
    Section, Size,
};
use pm_shell::{Extension, ExtensionAction, Tone, Workspace};

use crate::actions::Actions;

/// Emits the extensions page for `workspace`.
pub fn extensions_page(workspace: &Workspace, actions: &Actions) {
    PageHeader::new("Extensions").show(|| {});
    PageBody::new().show(|| {
        let needle = FilterField::new("Search extensions")
            .size(Size::Lg)
            .modifier(Modifier::new().fill_max_width())
            .show();
        let Some(extensions) = &workspace.extensions else {
            Label::new("Asking the server which extensions there are…")
                .variant(LabelVariant::Subtle)
                .show();
            return;
        };
        let listed: Vec<&Extension> = extensions
            .iter()
            .filter(|extension| extension.matches(&needle))
            .collect();
        if listed.is_empty() {
            Label::new("No extension matches.")
                .variant(LabelVariant::Subtle)
                .show();
            return;
        }
        let (installed, available): (Vec<&Extension>, Vec<&Extension>) = listed
            .into_iter()
            .partition(|extension| extension.installed);
        Section::new("Installed on this machine").show(installed.len(), |index| {
            extension_row(installed[index], actions)
        });
        Section::new("Available").show(available.len(), |index| {
            extension_row(available[index], actions)
        });
    });
}

/// Emits the row of `extension`: its mark, name and version over what it is
/// for and how it stands, and the button that acts on it.
fn extension_row(extension: &Extension, actions: &Actions) {
    let scheme = color_scheme();
    let press = actions.press_extension.clone();
    let id = extension.id.clone();
    let variant = match extension.action {
        ExtensionAction::Install | ExtensionAction::Update => ButtonVariant::Secondary,
        ExtensionAction::Uninstall | ExtensionAction::Installing => ButtonVariant::Ghost,
    };
    let button = Button::new(extension.action.label())
        .variant(variant)
        .size(Size::Sm)
        .disabled(extension.action == ExtensionAction::Installing)
        .icon_when(
            extension.action == ExtensionAction::Installing,
            phosphor::CIRCLE_NOTCH,
        )
        .spinning(extension.action == ExtensionAction::Installing)
        .on_click(move || press(id.clone()));
    let mut row = ItemRow::new(extension.name.clone()).icon(if extension.package {
        phosphor::PLUG
    } else {
        phosphor::FILE_CODE
    });
    if !extension.version.is_empty() {
        row = row.code_tag(extension.version.clone());
    }
    if extension.local {
        row = row.tag("your own");
    }
    if !extension.about.is_empty() {
        row = row.line(extension.about.clone());
    }
    for line in &extension.provides {
        row = row.faint_line(line.clone());
    }
    if let Some(owner) = &extension.owner {
        row = row.faint_line(owner.clone());
    }
    if !extension.standing.is_empty() {
        let colour = match extension.tone {
            Tone::Muted => scheme.on_surface_variant,
            Tone::Subtle => scheme.on_surface_variant,
            Tone::Accent => scheme.primary,
            Tone::Danger => scheme.error,
        };
        row = row.note(extension.standing.clone(), colour);
    }
    row.action(button).show();
}
