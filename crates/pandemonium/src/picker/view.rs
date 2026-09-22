//! The picker as it is drawn: a field over a list, centred over the window.
//!
//! One screen draws every list the window asks a reader to choose from, and
//! the two prompts that ask for a line of text instead — a prompt is the
//! same panel with nothing under the field. What fills the list is the
//! window's; how it reads is here.

use pm_ui::{Div, Styled, Theme, field, h_flex, rule, text, v_flex};

use crate::message::Message;
use crate::picker::state::{Kind, Picker};

/// How wide the panel is drawn.
const WIDTH: f32 = 620.0;

/// How far from the top of the window it hangs.
pub const TOP: f32 = 96.0;

/// Height of one row of the list.
const ROW_HEIGHT: f32 = 30.0;

/// Most rows drawn at once, however many the query leaves.
const VISIBLE: usize = 14;

/// Builds the panel for `picker`, over whatever the window is showing.
pub fn picker(theme: &Theme, picker: &Picker) -> Div<Message> {
    let prompt = picker.kind().is_prompt();

    v_flex()
        .w_px(WIDTH)
        .items_stretch()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .child(
            field(picker.field().value(), picker.field().caret(), true)
                .placeholder(picker.kind().placeholder())
                .w_full()
                .px(2)
                .py(1.5)
                .on_press(Message::PlacePicker),
        )
        .when(!prompt, |panel| {
            panel.child(rule(theme)).child(rows(theme, picker))
        })
        .when(prompt, |panel| panel.child(hint(theme, picker.kind())))
}

/// Builds the list of what the query leaves.
fn rows(theme: &Theme, picker: &Picker) -> Div<Message> {
    let first = picker.selected().saturating_sub(VISIBLE - 1);
    let shown = picker
        .shown()
        .skip(first)
        .take(VISIBLE)
        .map(|(place, row)| self::row(theme, place, row, place == picker.selected()))
        .collect::<Vec<_>>();
    let empty = shown.is_empty();

    v_flex()
        .w_full()
        .py(0.5)
        .items_stretch()
        .when(empty, |list| {
            list.child(
                text("No matches")
                    .text_sm()
                    .font_light()
                    .color(theme.colors.text_subtle)
                    .px(2)
                    .py(1.5),
            )
        })
        .children(shown)
}

/// Builds one row of the list, lit while it is the selected one.
fn row(
    theme: &Theme,
    place: usize,
    row: &crate::picker::state::Row,
    selected: bool,
) -> Div<Message> {
    let color = if row.enabled {
        theme.colors.text
    } else {
        theme.colors.text_subtle
    };

    h_flex()
        .w_full()
        .h_px(ROW_HEIGHT)
        .px(2)
        .gap(1)
        .items_center()
        .overflow_hidden()
        .when(selected, |line| line.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ChoosePicker(place))
        .child(text(row.label.clone()).text_sm().color(color))
        .child(h_flex().flex_1())
        .child(
            text(row.detail.clone())
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// Builds the line under a prompt saying what it will do with what is typed.
fn hint(theme: &Theme, kind: Kind) -> Div<Message> {
    let label = match kind {
        Kind::Line => "Enter a line number, or a line and column",
        Kind::Rename => "Enter the new name, everywhere the symbol is used",
        Kind::NewFile | Kind::NewFolder => "A name with slashes in it makes the directories too",
        Kind::RenamePath => "Enter the new name for the file on disk",
        _ => "This cannot be undone",
    };

    h_flex().w_full().px(2).pb(1.5).child(
        text(label)
            .text_xs()
            .font_light()
            .color(theme.colors.text_subtle),
    )
}
