//! The dialogs drawn over the window: the offer of a newer release and the code a sign-in waits for.

use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, Dialog, DialogVariant, Label, LabelVariant, Markdown, Size,
};
use pm_shell::{Block, RUNNING, SessionTab, SignInCode, TabAction, UpdateOffer, Workspace};

use crate::actions::{Actions, SignInAction, UpdateAction};

/// Emits whichever dialog `workspace` shows, the sign-in code over the update offer.
pub fn dialogs(workspace: &Workspace, actions: &Actions) {
    if let Some(offer) = workspace.update.as_ref().filter(|offer| offer.open) {
        update_dialog(offer, actions);
    }
    if let Some(code) = &workspace.sign_in {
        sign_in_dialog(code, actions);
    }
}

/// Emits the dialog offering the newer release `offer`: its version, its notes and the buttons that install it or put it off.
fn update_dialog(offer: &UpdateOffer, actions: &Actions) {
    let busy = offer.run.busy();
    let dismiss = actions.update.clone();
    let mut dialog = Dialog::new(format!("{} is available", offer.name)).size(Size::Lg);
    if busy {
        dialog = dialog.variant(DialogVariant::Blocking);
    }
    dialog
        .on_dismiss(move || dismiss(UpdateAction::Close))
        .show(|| {
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
            if let Some(notes) = &offer.notes {
                let scroll = remember_scroll_state();
                column_with(
                    Modifier::new()
                        .fill_max_width()
                        .height_in(0.0, 320.0)
                        .vertical_scroll(&scroll),
                    || {
                        if let Block::Markdown { blocks, .. } = Block::markdown(notes.clone()) {
                            Markdown::new(blocks).size(Size::Sm).show();
                        }
                    },
                );
            }
            if let Some(status) = offer.run.status() {
                Label::new(status)
                    .variant(LabelVariant::Muted)
                    .wrap()
                    .show();
            }
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(pm_components::spacing(8.0)),
                Alignment::Center,
                || {
                    update_button(
                        actions,
                        "View on GitHub",
                        UpdateAction::ViewPage,
                        ButtonVariant::Ghost,
                    );
                    let tab = actions.file_tab.clone();
                    let close = actions.update.clone();
                    Button::new("Open in a tab")
                        .variant(ButtonVariant::Ghost)
                        .size(Size::Sm)
                        .on_click(move || {
                            close(UpdateAction::Close);
                            tab(TabAction::Open(SessionTab::Update));
                        })
                        .show();
                    spacer(Modifier::new().weight(1.0));
                    let later = if busy { "Cancel" } else { "Later" };
                    update_button(
                        actions,
                        later,
                        UpdateAction::Close,
                        ButtonVariant::Secondary,
                    );
                    if !busy {
                        update_button(
                            actions,
                            "Update and Restart",
                            UpdateAction::Install,
                            ButtonVariant::Primary,
                        );
                    }
                },
            );
        });
}

/// A button of the update dialog labelled `label`, in `variant`, asking for `action`.
#[track_caller]
pub(crate) fn update_button(
    actions: &Actions,
    label: &str,
    action: UpdateAction,
    variant: ButtonVariant,
) {
    let update = actions.update.clone();
    Button::new(label)
        .variant(variant)
        .size(Size::Sm)
        .on_click(move || update(action))
        .show();
}

/// Emits the dialog showing the code `code` a sign-in waits for, with buttons that copy it and open the page it is entered on.
fn sign_in_dialog(code: &SignInCode, actions: &Actions) {
    let hide = actions.sign_in.clone();
    Dialog::new(format!("Sign in to {}", code.agent))
        .size(Size::Sm)
        .on_dismiss(move || hide(SignInAction::Hide))
        .show(|| {
            Label::new(format!(
                "Enter this code on {} to finish signing in. This closes by itself once you have.",
                code.url
            ))
            .variant(LabelVariant::Muted)
            .wrap()
            .show();
            Label::new(code.code.clone())
                .variant(LabelVariant::Code)
                .size(Size::Lg)
                .show();
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(pm_components::spacing(8.0)),
                Alignment::Center,
                || {
                    sign_in_button(actions, "Hide", SignInAction::Hide, ButtonVariant::Ghost);
                    spacer(Modifier::new().weight(1.0));
                    sign_in_button(
                        actions,
                        "Copy code",
                        SignInAction::Copy,
                        ButtonVariant::Secondary,
                    );
                    sign_in_button(
                        actions,
                        "Open page",
                        SignInAction::Open,
                        ButtonVariant::Primary,
                    );
                },
            );
        });
}

/// A button of the sign-in code dialog labelled `label`, in `variant`, asking for `action`.
#[track_caller]
fn sign_in_button(actions: &Actions, label: &str, action: SignInAction, variant: ButtonVariant) {
    let sign_in = actions.sign_in.clone();
    Button::new(label)
        .variant(variant)
        .size(Size::Sm)
        .on_click(move || sign_in(action))
        .show();
}
