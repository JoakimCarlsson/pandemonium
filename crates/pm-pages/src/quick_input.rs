//! The quick input, opened from the command centre: a modal near the top of
//! the window holding a pick list of files, commands, symbols, outline
//! items, lines, sessions or choices, with the keys bound to each command.

use rustle::prelude::*;

use pm_components::{Modal, ModalVariant, PickItem, PickList, Size, file_glyph};
use pm_shell::{QuickAction, QuickInput, QuickItem, QuickKind, QuickMode, Workspace, quick_items};

use crate::actions::Actions;

/// How wide the quick input is.
const QUICK_WIDTH: f32 = 600.0;

/// Emits the quick input of `workspace`, when it is open.
pub fn quick_input(workspace: &Workspace, actions: &Actions) {
    let Some(quick) = &workspace.quick else {
        return;
    };
    let items: Vec<PickItem> = quick_items(workspace).iter().map(pick_item).collect();
    let close = actions.quick.clone();
    let escape = actions.quick.clone();
    let typed = actions.quick.clone();
    let picked = actions.quick.clone();
    let moved = actions.quick.clone();
    let empty = empty_note(quick);
    Modal::new()
        .variant(ModalVariant::Top)
        .size(Size::Lg)
        .width(QUICK_WIDTH)
        .show(
            move || close(QuickAction::Close),
            || {
                PickList::new(items)
                    .query(quick.query.clone())
                    .placeholder(quick.placeholder())
                    .selected(quick.selected)
                    .empty(empty)
                    .visible(12)
                    .footer()
                    .modifier(Modifier::new().fill_max_width())
                    .on_query(move |text| typed(QuickAction::Query(text)))
                    .on_pick(move |place| picked(QuickAction::Pick(Some(place))))
                    .on_move(move |by| moved(QuickAction::Move(by)))
                    .on_close(move || escape(QuickAction::Close))
                    .show();
            },
        );
}

/// What the list says when it lists nothing.
fn empty_note(quick: &QuickInput) -> String {
    if quick.waiting.is_some() {
        return "Looking…".to_owned();
    }
    if quick.choosing.is_some() {
        return "No choice matches.".to_owned();
    }
    match (quick.mode(), quick.needle().is_empty()) {
        (QuickMode::Symbols, true) => "Type the name of a symbol of the project.".to_owned(),
        (QuickMode::Outline, true) => "Type the name of a symbol of the open file.".to_owned(),
        (QuickMode::Text, true) => "Type the text to find in the project.".to_owned(),
        (QuickMode::Files, _) => "No file matches.".to_owned(),
        (QuickMode::Commands, _) => "No command matches.".to_owned(),
        _ => "Nothing found.".to_owned(),
    }
}

/// The pick list row of `item`.
fn pick_item(item: &QuickItem) -> PickItem {
    let scheme = color_scheme();
    let (icon, tint) = match item.kind {
        QuickKind::File => file_glyph(&item.title),
        QuickKind::Command => (phosphor::COMMAND, None),
        QuickKind::Symbol => (phosphor::CUBE, Some(scheme.primary)),
        QuickKind::Text => (phosphor::TEXT_ALIGN_LEFT, None),
        QuickKind::Session => (phosphor::CHAT_CIRCLE, None),
        QuickKind::Choice | QuickKind::Current => {
            (phosphor::CIRCLE, Some(scheme.on_surface_variant))
        }
    };
    PickItem {
        title: item.title.clone(),
        detail: item.detail.clone(),
        icon: Some(icon),
        tint,
        keys: item.keys.clone(),
        matched: item.matched.clone(),
        current: item.kind == QuickKind::Current,
    }
}
