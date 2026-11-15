//! The adder: a popover under the sidebar's "Workspaces" heading that adds a
//! project from a folder, chosen in the folder picker, or from a clone of a
//! repository on GitHub or any other git host, into a folder and on a
//! machine of the reader's choosing.

use pm_components::{
    Button, ButtonVariant, Label, LabelVariant, Leading, Notice, NoticeVariant, Popover, Size,
    TextInput, Tile,
};
use pm_shell::{AddStep, Adder, PickerAction, ProjectAction, clone_address, clone_name};
use rustle::prelude::*;

use crate::actions::Actions;

/// How wide the adder is.
const WIDTH: f32 = 280.0;

/// Emits the adder in `state`, calling `actions` when the reader acts.
pub fn adder(state: &Adder, actions: &Actions) {
    let close = actions.projects.clone();
    Popover::new()
        .width(WIDTH)
        .offset(8.0, 0.0)
        .on_dismiss(move || close(ProjectAction::Adding(None)))
        .show(|| match state.step {
            AddStep::Choose => choices(actions),
            AddStep::Clone => clone_form(state, actions),
        });
}

/// The two ways of adding a project.
fn choices(actions: &Actions) {
    let pick = actions.projects.clone();
    let clone = actions.projects.clone();
    column_with(Modifier::new().fill_max_width().padding(4.0), || {
        Tile::new("Open a folder…")
            .leading(Leading::Icon(phosphor::FOLDER_OPEN))
            .on_click(move || pick(ProjectAction::Picker(PickerAction::Open(None))))
            .show();
        Tile::new("Clone a repository…")
            .leading(Leading::Icon(phosphor::GITHUB_LOGO))
            .on_click(move || clone(ProjectAction::Adding(Some(AddStep::Clone))))
            .show();
    });
}

/// The machines the clone can be made on, the one chosen raised.
fn machine_choice(state: &Adder, actions: &Actions) {
    flow_row_with(
        Modifier::new().fill_max_width(),
        FlowOptions::new(6.0, 6.0),
        || {
            for machine in &state.machines {
                let pick = actions.projects.clone();
                let host = machine.host.clone();
                let chosen = state.machine == machine.host;
                key(machine.name.clone(), || {
                    Button::new(machine.name.clone())
                        .icon(if machine.host.is_some() {
                            phosphor::CLOUD
                        } else {
                            phosphor::DESKTOP
                        })
                        .variant(if chosen {
                            ButtonVariant::Secondary
                        } else {
                            ButtonVariant::Ghost
                        })
                        .size(Size::Xs)
                        .on_click(move || pick(ProjectAction::CloneOn(host.clone())))
                        .show();
                });
            }
        },
    );
}

/// How many repositories the clone form lists before it scrolls.
const REPOSITORIES_SHOWN: f32 = 6.0;

/// How tall a repository's row is.
const REPOSITORY_ROW: f32 = 26.0;

/// The GitHub accounts the chosen machine is signed in to, the one listed raised, and its repositories that hold what the address field holds; picking one fills the address.
fn github_picker(state: &Adder, actions: &Actions) {
    let Some(owners) = &state.owners else {
        spacer(Modifier::new().height(pm_components::spacing(6.0)));
        Label::new("Reading your GitHub accounts…")
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
        return;
    };
    if let Some(error) = &state.github_error {
        spacer(Modifier::new().height(pm_components::spacing(6.0)));
        Label::new(format!("GitHub isn't listed: {error}"))
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
    }
    if owners.is_empty() {
        return;
    }
    spacer(Modifier::new().height(pm_components::spacing(8.0)));
    flow_row_with(
        Modifier::new().fill_max_width(),
        FlowOptions::new(6.0, 6.0),
        || {
            for (login, organization) in owners {
                let pick = actions.projects.clone();
                let owner = login.clone();
                let chosen = state.owner.as_ref() == Some(login);
                key(login.clone(), || {
                    Button::new(login.clone())
                        .icon(if *organization {
                            phosphor::BUILDINGS
                        } else {
                            phosphor::USER
                        })
                        .variant(if chosen {
                            ButtonVariant::Secondary
                        } else {
                            ButtonVariant::Ghost
                        })
                        .size(Size::Xs)
                        .on_click(move || pick(ProjectAction::CloneOwner(owner.clone())))
                        .show();
                });
            }
        },
    );
    spacer(Modifier::new().height(pm_components::spacing(6.0)));
    if state.repositories.is_none() {
        Label::new("Listing repositories…")
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
        return;
    }
    let listed = state.matching_repositories();
    if listed.is_empty() {
        Label::new("No repository matches.")
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
        return;
    }
    let height = listed.len().min(REPOSITORIES_SHOWN as usize) as f32 * REPOSITORY_ROW;
    let rows: Vec<_> = listed.into_iter().cloned().collect();
    let scroll = remember_scroll_state();
    column_with(
        Modifier::new()
            .fill_max_width()
            .height(height)
            .vertical_scroll(&scroll),
        || {
            for repository in rows {
                let pick = actions.projects.clone();
                let url = repository.url.clone();
                let chosen = state.url == repository.url;
                key(repository.name.clone(), || {
                    let mut row = Tile::new(repository.name.clone())
                        .leading(Leading::Icon(if repository.private {
                            phosphor::LOCK_SIMPLE
                        } else {
                            phosphor::BOOK_BOOKMARK
                        }))
                        .size(Size::Xs)
                        .selected(chosen)
                        .on_click(move || pick(ProjectAction::CloneUrl(url.clone())));
                    if repository.private {
                        row = row.trailing("private");
                    }
                    row.show();
                });
            }
        },
    );
}

/// The form that clones a repository, into the folder and on the machine chosen, and adds the clone.
fn clone_form(state: &Adder, actions: &Actions) {
    let change = actions.projects.clone();
    let submit = actions.projects.clone();
    let back = actions.projects.clone();
    let start = actions.projects.clone();
    let address = clone_address(&state.url);
    let ready = !address.is_empty() && !state.busy;
    column_with(Modifier::new().fill_max_width().padding(12.0), || {
        Label::new("Clone a repository").size(Size::Md).show();
        spacer(Modifier::new().height(pm_components::spacing(4.0)));
        Label::new("A GitHub owner/name, or any git address.")
            .variant(LabelVariant::Muted)
            .size(Size::Xs)
            .show();
        spacer(Modifier::new().height(pm_components::spacing(10.0)));
        TextInput::new(state.url.clone())
            .placeholder("owner/name or https://…")
            .autofocus()
            .size(Size::Sm)
            .modifier(Modifier::new().fill_max_width())
            .on_change(move |text| change(ProjectAction::CloneUrl(text)))
            .on_submit(move |_| submit(ProjectAction::Clone))
            .show();
        github_picker(state, actions);
        if !address.is_empty() {
            spacer(Modifier::new().height(pm_components::spacing(6.0)));
            Label::new(format!("Into a folder named {}", clone_name(&address)))
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .show();
        }
        spacer(Modifier::new().height(pm_components::spacing(8.0)));
        let under = actions.projects.clone();
        TextInput::new(state.under.clone())
            .placeholder("Inside the usual folder for clones")
            .size(Size::Sm)
            .modifier(Modifier::new().fill_max_width())
            .on_change(move |text| under(ProjectAction::CloneUnder(text)))
            .show();
        if state.machines.len() > 1 {
            spacer(Modifier::new().height(pm_components::spacing(8.0)));
            machine_choice(state, actions);
        }
        if let Some(error) = &state.error {
            spacer(Modifier::new().height(pm_components::spacing(8.0)));
            Notice::new("The clone failed")
                .detail(error.clone())
                .variant(NoticeVariant::Danger)
                .size(Size::Sm)
                .show();
        }
        spacer(Modifier::new().height(pm_components::spacing(12.0)));
        row_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::End,
            Alignment::Center,
            || {
                Button::new("Back")
                    .variant(ButtonVariant::Secondary)
                    .size(Size::Sm)
                    .on_click(move || back(ProjectAction::Adding(Some(AddStep::Choose))))
                    .show();
                spacer(Modifier::new().width(pm_components::spacing(8.0)));
                Button::new(if state.busy { "Cloning…" } else { "Clone" })
                    .icon(phosphor::DOWNLOAD_SIMPLE)
                    .variant(ButtonVariant::Primary)
                    .size(Size::Sm)
                    .disabled(!ready)
                    .on_click(move || start(ProjectAction::Clone))
                    .show();
            },
        );
    });
}
