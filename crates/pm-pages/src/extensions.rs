//! The extensions page: what the server on this machine has installed, then
//! what it can install, narrowed by a search, each a row saying what the
//! extension is for and how it stands, with the button that installs,
//! updates or uninstalls it.

use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, Dialog, FilterField, ItemRow, Label, LabelVariant, Notice,
    NoticeVariant, PageBody, PageHeader, Section, Size, Switch,
};
use pm_shell::{
    Capability, Extension, ExtensionAction, ExtensionDo, ExtensionPreview, Tone, Workspace,
};

use crate::actions::Actions;

/// Emits the extensions page for `workspace`.
pub fn extensions_page(workspace: &Workspace, actions: &Actions) {
    let from_folder = actions.extension.clone();
    PageHeader::new("Extensions").show(move || {
        Button::new("Install from folder…")
            .icon(phosphor::FOLDER_OPEN)
            .variant(ButtonVariant::Secondary)
            .size(Size::Sm)
            .on_click(move || from_folder(ExtensionDo::InstallFromFolder))
            .show();
    });
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
            extension_row(installed[index], actions);
            let extension = installed[index];
            let grant = actions.extension.clone();
            let id = extension.id.clone();
            capability_switches(&extension.capabilities, move |capability, granted| {
                grant(ExtensionDo::Grant(id.clone(), capability, granted))
            });
        });
        Section::new("Available").show(available.len(), |index| {
            extension_row(available[index], actions)
        });
    });
}

/// Emits a switch for each of `capabilities` an extension asks, saying what it lets the extension do, calling `on_change` with the capability's name and whether it is now allowed.
pub(crate) fn capability_switches(
    capabilities: &[Capability],
    on_change: impl Fn(String, bool) + Clone + 'static,
) {
    for capability in capabilities {
        let change = on_change.clone();
        let id = capability.id.clone();
        key(capability.id.clone(), || {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .padding_each(40.0, 2.0, 12.0, 2.0),
                Arrangement::SpacedBy(pm_components::spacing(8.0)),
                Alignment::Center,
                || {
                    Label::new(capability.about.clone())
                        .variant(LabelVariant::Muted)
                        .size(Size::Xs)
                        .modifier(Modifier::new().weight(1.0))
                        .show();
                    Switch::new(capability.granted)
                        .size(Size::Xs)
                        .on_change(move |granted| change(id.clone(), granted))
                        .show();
                },
            );
        });
    }
}

/// Emits the dialog showing the extension in the folder `preview` holds before it is installed: what it carries and asks to be allowed, with a switch for each, and the buttons that install it or give up.
pub fn extension_preview_dialog(preview: &ExtensionPreview, actions: &Actions) {
    let (cancel, install, grant) = (
        actions.extension.clone(),
        actions.extension.clone(),
        actions.extension.clone(),
    );
    let dismiss = cancel.clone();
    Dialog::new("Install extension")
        .size(Size::Sm)
        .on_dismiss(move || dismiss(ExtensionDo::CancelPreview))
        .show(|| {
            Label::new(preview.path.clone())
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .show();
            if let Some(problem) = &preview.problem {
                Notice::new("This folder can't be installed")
                    .detail(problem.clone())
                    .variant(NoticeVariant::Danger)
                    .size(Size::Sm)
                    .show();
            }
            match &preview.extension {
                None if preview.problem.is_none() => Label::new("Reading the folder…")
                    .variant(LabelVariant::Subtle)
                    .show(),
                None => {}
                Some(extension) => {
                    Label::new(match extension.version.is_empty() {
                        true => extension.name.clone(),
                        false => format!("{} {}", extension.name, extension.version),
                    })
                    .size(Size::Md)
                    .show();
                    if !extension.about.is_empty() {
                        Label::new(extension.about.clone())
                            .variant(LabelVariant::Muted)
                            .size(Size::Sm)
                            .show();
                    }
                    for line in &extension.provides {
                        Label::new(line.clone())
                            .variant(LabelVariant::Subtle)
                            .size(Size::Xs)
                            .show();
                    }
                    if !extension.capabilities.is_empty() {
                        Label::new("It asks to be allowed to:")
                            .size(Size::Sm)
                            .show();
                        capability_switches(&extension.capabilities, move |capability, granted| {
                            grant(ExtensionDo::PreviewGrant(capability, granted))
                        });
                    }
                }
            }
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::End,
                Alignment::Center,
                || {
                    Button::new("Cancel")
                        .variant(ButtonVariant::Secondary)
                        .size(Size::Sm)
                        .on_click(move || cancel(ExtensionDo::CancelPreview))
                        .show();
                    spacer(Modifier::new().width(pm_components::spacing(8.0)));
                    Button::new("Install")
                        .icon(phosphor::DOWNLOAD_SIMPLE)
                        .variant(ButtonVariant::Primary)
                        .size(Size::Sm)
                        .disabled(preview.extension.is_none())
                        .on_click(move || install(ExtensionDo::InstallPreview))
                        .show();
                },
            );
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
