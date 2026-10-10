//! The parts the list pages of the settings pane are built from: a header
//! that opens and folds a list, the bordered box its rows sit in, a row, a
//! badge, and a line that only says something.

use pm_gfx::Rgba;
use pm_ui::{Div, IconName, Styled, Theme, h_flex, icon, rule, text, v_flex};

use crate::message::Message;

/// The most characters of a description a row shows.
const DESCRIPTION: usize = 140;

/// A small label in a rounded wash, in `tone`.
pub(super) fn badge(theme: &Theme, label: &str, tone: Rgba) -> Div<Message> {
    h_flex()
        .px(1.5)
        .rounded(theme.radius.lg)
        .bg(theme.colors.surface_selected)
        .child(text(label.to_owned()).text_xs().color(tone))
}

/// The title of a list: a chevron that opens or folds it, its name, and how
/// many it holds.
pub(super) fn header(
    theme: &Theme,
    title: &str,
    count: usize,
    open: bool,
    toggle: Message,
) -> Div<Message> {
    let chevron = match open {
        true => IconName::ChevronDown,
        false => IconName::ChevronRight,
    };
    h_flex()
        .h_px(theme.size.control)
        .gap(1.5)
        .items_center()
        .on_click(toggle)
        .child(icon(chevron).color(theme.colors.text_muted))
        .child(text(title).font_semibold())
        .child(
            h_flex()
                .px(1.5)
                .rounded(theme.radius.lg)
                .bg(theme.colors.surface_selected)
                .child(
                    text(count.to_string())
                        .text_xs()
                        .color(theme.colors.text_muted),
                ),
        )
}

/// Rows in a bordered box, a hairline between each and the next.
pub(super) fn card(theme: &Theme, rows: Vec<Div<Message>>) -> Div<Message> {
    let mut body = v_flex()
        .w_full()
        .rounded(theme.radius.md)
        .border_1(theme.colors.border_variant)
        .bg(theme.colors.surface);
    for (place, row) in rows.into_iter().enumerate() {
        if place > 0 {
            body = body.child(rule(theme));
        }
        body = body.child(row);
    }
    body
}

/// One row of a card: a name over what it is, and `control` at the end.
pub(super) fn row(theme: &Theme, title: &str, detail: &str, control: Div<Message>) -> Div<Message> {
    h_flex()
        .w_full()
        .px(3)
        .py(2.5)
        .gap(4)
        .items_center()
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .gap(0.5)
                .overflow_hidden()
                .child(text(title).font_medium())
                .child(text(detail).text_sm().color(theme.colors.text_muted)),
        )
        .child(control)
}

/// A row of a card that only says something.
pub(super) fn note(theme: &Theme, message: &str) -> Div<Message> {
    h_flex()
        .w_full()
        .px(3)
        .py(3)
        .child(text(message).text_sm().color(theme.colors.text_muted))
}

/// `description` cut to its first line and to what a row has room for.
pub(super) fn clipped(description: &str) -> String {
    let line = description.lines().next().unwrap_or_default().trim();
    match line.chars().count() > DESCRIPTION {
        true => format!("{}…", line.chars().take(DESCRIPTION).collect::<String>()),
        false => match line.is_empty() {
            true => "No description provided.".to_owned(),
            false => line.to_owned(),
        },
    }
}
