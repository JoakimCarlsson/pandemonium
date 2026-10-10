//! The search bar a pane wears while something is being looked for in it.
//!
//! The bar is part of the editor screen rather than a dock of its own: it
//! sits between the pane's tabs and its text, belongs to the one pane it was
//! opened over, and every control on it resolves to the same [`Action`] a
//! keybinding would — so Find Next is one command however it was asked for.

use pm_ui::{Div, Styled, Theme, h_flex, text, v_flex};

use crate::editor::search::{Search, SearchField};
use crate::input::hinted_input_view;
use crate::keymap::Action;
use crate::message::Message;
use crate::panes::PaneId;

/// Height of one row of the bar.
const ROW_HEIGHT: f32 = 30.0;

/// Widest the count of matches is drawn, so the controls beside it hold still.
const COUNT_WIDTH: f32 = 72.0;

/// Builds the bar for `search`, over the pane it was opened in.
pub fn search_bar(theme: &Theme, pane: PaneId, search: &Search, solid: bool) -> Div<Message> {
    v_flex()
        .w_full()
        .px(1)
        .py(0.5)
        .gap(0.5)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .child(query_row(theme, pane, search, solid))
        .when_some(search.error(), |bar, error| {
            bar.child(text(error.to_owned()).text_xs().color(theme.colors.danger))
        })
        .when(search.is_replacing(), |bar| {
            bar.child(replacement_row(theme, pane, search, solid))
        })
}

/// Builds the row holding what is being looked for and where it was found.
fn query_row(theme: &Theme, pane: PaneId, search: &Search, solid: bool) -> Div<Message> {
    let focused = search.field() == SearchField::Query;

    h_flex()
        .w_full()
        .h_px(ROW_HEIGHT)
        .gap(0.5)
        .items_center()
        .child(toggle(
            theme,
            "⇅",
            search.is_replacing(),
            Message::ToggleSearchReplace(pane),
        ))
        .child(
            hinted_input_view(
                theme,
                search.query(),
                focused,
                solid,
                "Find (one line at a time)",
                move |phase, anchor, head| {
                    Message::WriteSearch(pane, SearchField::Query, phase, anchor, head)
                },
                Message::ShowInputMenu,
            )
            .flex_1()
            .h_px(ROW_HEIGHT - 6.0)
            .border_1(if focused {
                theme.colors.border_focused
            } else {
                theme.colors.border
            }),
        )
        .child(toggle(
            theme,
            ".*",
            search.is_regex(),
            Message::ToggleSearchRegex(pane),
        ))
        .child(toggle(
            theme,
            "Aa",
            search.is_case_sensitive(),
            Message::ToggleSearchCase(pane),
        ))
        .child(toggle(
            theme,
            "ab",
            search.is_whole_word(),
            Message::ToggleSearchWord(pane),
        ))
        .child(standing(theme, search))
        .child(command(theme, pane, "↑", Action::FindPrevious))
        .child(command(theme, pane, "↓", Action::FindNext))
        .child(close(theme, pane))
}

/// Builds the row holding what the matches are replaced with.
fn replacement_row(theme: &Theme, pane: PaneId, search: &Search, solid: bool) -> Div<Message> {
    let focused = search.field() == SearchField::Replacement;

    h_flex()
        .w_full()
        .h_px(ROW_HEIGHT)
        .gap(0.5)
        .items_center()
        .child(v_flex().w_px(ROW_HEIGHT))
        .child(
            hinted_input_view(
                theme,
                search.replacement(),
                focused,
                solid,
                "Replace",
                move |phase, anchor, head| {
                    Message::WriteSearch(pane, SearchField::Replacement, phase, anchor, head)
                },
                Message::ShowInputMenu,
            )
            .flex_1()
            .h_px(ROW_HEIGHT - 6.0)
            .border_1(if focused {
                theme.colors.border_focused
            } else {
                theme.colors.border
            }),
        )
        .child(command(theme, pane, "Replace", Action::ReplaceMatch))
        .child(command(theme, pane, "All", Action::ReplaceAll))
}

/// Builds the reading of which match is being looked at, of how many.
fn standing(theme: &Theme, search: &Search) -> Div<Message> {
    let label = match search.standing() {
        Some((at, of)) => format!("{at} of {of}"),
        None if search.query().is_empty() => String::new(),
        None => {
            if search.error().is_some() {
                String::new()
            } else {
                "No results".to_owned()
            }
        }
    };

    h_flex()
        .w_px(COUNT_WIDTH)
        .justify_end()
        .items_center()
        .overflow_hidden()
        .child(
            text(label)
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// Builds one of the bar's switches, lit while it is on.
fn toggle(theme: &Theme, label: &str, on: bool, message: Message) -> Div<Message> {
    let color = if on {
        theme.colors.text_on_accent
    } else {
        theme.colors.text_muted
    };

    h_flex()
        .h_px(ROW_HEIGHT - 8.0)
        .px(0.75)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .when(on, |control| control.bg(theme.colors.accent))
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(message)
        .child(text(label.to_owned()).text_xs().font_light().color(color))
}

/// Builds the control that puts the bar away.
fn close(theme: &Theme, pane: PaneId) -> Div<Message> {
    h_flex()
        .h_px(ROW_HEIGHT - 8.0)
        .px(0.75)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(Message::CloseSearch(pane))
        .child(
            text("×")
                .text_xs()
                .font_light()
                .color(theme.colors.text_muted),
        )
}

/// Builds one of the bar's buttons, which runs `action` in `pane`.
fn command(theme: &Theme, pane: PaneId, label: &str, action: Action) -> Div<Message> {
    h_flex()
        .h_px(ROW_HEIGHT - 8.0)
        .px(0.75)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(Message::PaneAction(pane, action))
        .child(
            text(label.to_owned())
                .text_xs()
                .font_light()
                .color(theme.colors.text_muted),
        )
}
