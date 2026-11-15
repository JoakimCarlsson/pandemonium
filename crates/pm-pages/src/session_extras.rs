//! The session view's tabs that are neither the conversation, the diff, a
//! file nor the browser: the editable excerpts of a search, edited in place
//! across files, and the notes of the newer release with the buttons that
//! install it.

use rustle::prelude::*;

use pm_components::{ButtonVariant, Label, LabelVariant, Markdown, Size};
use pm_shell::{Block, RUNNING, UpdateOffer, Workspace};

use crate::actions::{Actions, UpdateAction};
use crate::dialogs::update_button;
use crate::search_panel::editable_excerpts;

/// Width of the column the release notes are read in.
const NOTES_WIDTH: f32 = 720.0;

/// Emits the Excerpts tab: the stretches around the hits of the search,
/// from every file, in editors written back to their files on saving.
pub fn excerpts_tab(workspace: &Workspace, actions: &Actions) {
    let scheme = color_scheme();
    let search = workspace.search.clone();
    let actions = actions.clone();
    let scroll = remember_scroll_state();
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .background(scheme.background)
            .vertical_scroll(&scroll)
            .padding_xy(16.0, 12.0),
        Arrangement::SpacedBy(8.0),
        Alignment::Start,
        move || {
            Label::new(format!("Excerpts of “{}”", search.query))
                .variant(LabelVariant::Heading)
                .size(Size::Sm)
                .show();
            if search.excerpts.is_empty() {
                Label::new("Search the project, then edit what it found to see it here.")
                    .variant(LabelVariant::Muted)
                    .size(Size::Sm)
                    .show();
                return;
            }
            editable_excerpts(&search, &actions);
        },
    );
}

/// Emits the Update tab for `offer`: the version, its notes and the buttons
/// that install it or show its page.
pub fn update_tab(offer: &UpdateOffer, actions: &Actions) {
    let scheme = color_scheme();
    let offer = offer.clone();
    let actions = actions.clone();
    let scroll = remember_scroll_state();
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .background(scheme.background)
            .vertical_scroll(&scroll)
            .padding_xy(0.0, 24.0),
        Arrangement::Start,
        Alignment::Center,
        move || {
            let offer = offer.clone();
            let actions = actions.clone();
            column_arranged(
                Modifier::new().width(NOTES_WIDTH),
                Arrangement::SpacedBy(12.0),
                Alignment::Start,
                move || {
                    Label::new(format!("{} is available", offer.name))
                        .variant(LabelVariant::Heading)
                        .size(Size::Lg)
                        .show();
                    let published = offer
                        .published
                        .as_ref()
                        .map(|date| format!(", published {date}"))
                        .unwrap_or_default();
                    Label::new(format!(
                        "This is {RUNNING}. Version {}{published} is out.",
                        offer.version
                    ))
                    .variant(LabelVariant::Muted)
                    .wrap()
                    .show();
                    row_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(8.0),
                        Alignment::Center,
                        || {
                            if !offer.run.busy() {
                                update_button(
                                    &actions,
                                    "Update and Restart",
                                    UpdateAction::Install,
                                    ButtonVariant::Primary,
                                );
                            }
                            update_button(
                                &actions,
                                "View on GitHub",
                                UpdateAction::ViewPage,
                                ButtonVariant::Ghost,
                            );
                        },
                    );
                    if let Some(status) = offer.run.status() {
                        Label::new(status)
                            .variant(LabelVariant::Muted)
                            .wrap()
                            .show();
                    }
                    spacer(
                        Modifier::new()
                            .fill_max_width()
                            .height(1.0)
                            .background(scheme.outline_variant),
                    );
                    match &offer.notes {
                        Some(notes) => {
                            if let Block::Markdown { blocks, .. } = Block::markdown(notes.clone()) {
                                Markdown::new(blocks).size(Size::Sm).show();
                            }
                        }
                        None => Label::new("This release has no notes.")
                            .variant(LabelVariant::Subtle)
                            .show(),
                    }
                },
            );
        },
    );
}
