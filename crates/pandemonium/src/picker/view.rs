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

/// Width of the branch popover attached to the status bar.
const BRANCH_WIDTH: f32 = 360.0;
/// Width of agent control choices beside their control.
const AGENT_WIDTH: f32 = 280.0;

/// How far from the top of the window it hangs.
pub const TOP: f32 = 96.0;

/// Height of one row of the list.
const ROW_HEIGHT: f32 = 30.0;

/// Most rows drawn at once, however many the query leaves.
const VISIBLE: usize = 14;

/// Most branch rows drawn in the compact status-bar popover.
const BRANCH_VISIBLE: usize = 9;

/// Approximate height of the picker's input and border.
const FIELD_HEIGHT: f32 = 49.0;

/// Approximate height of the explanatory line under a prompt.
const HINT_HEIGHT: f32 = 32.0;

/// Width of a picker, with branch workflows using their compact popover size.
pub fn width(kind: Kind) -> f32 {
    match kind {
        Kind::Branches | Kind::NewBranch => BRANCH_WIDTH,
        Kind::Modes | Kind::Knob => AGENT_WIDTH,
        _ => WIDTH,
    }
}

/// Height occupied by the visible portion of `picker`.
pub fn height(picker: &Picker) -> f32 {
    if matches!(picker.kind(), Kind::Modes | Kind::Knob) {
        return picker.shown_count().clamp(1, VISIBLE) as f32 * ROW_HEIGHT + 8.0;
    }
    if picker.kind() == Kind::Branches {
        let shown = picker.shown().take(BRANCH_VISIBLE).collect::<Vec<_>>();
        let sections = shown
            .iter()
            .filter_map(|(_, row)| row.section)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let creating = usize::from(!picker.field().value().trim().is_empty());
        return FIELD_HEIGHT
            + shown.len().max(1) as f32 * 46.0
            + sections as f32 * 28.0
            + creating as f32 * 54.0;
    }
    if picker.kind().is_prompt() {
        return FIELD_HEIGHT + HINT_HEIGHT;
    }
    let visible = visible_rows(picker.kind());
    FIELD_HEIGHT + picker.shown_count().min(visible).max(1) as f32 * ROW_HEIGHT
}

/// How many rows this kind of picker shows at once.
fn visible_rows(kind: Kind) -> usize {
    match kind {
        Kind::Branches => BRANCH_VISIBLE,
        _ => VISIBLE,
    }
}

/// Builds the panel for `picker`, over whatever the window is showing.
pub fn picker(theme: &Theme, picker: &Picker) -> Div<Message> {
    let prompt = picker.kind().is_prompt();

    if matches!(picker.kind(), Kind::Modes | Kind::Knob) {
        return v_flex()
            .w_px(width(picker.kind()))
            .overflow_hidden()
            .bg(theme.colors.surface)
            .border_1(theme.colors.border)
            .rounded(theme.radius.lg)
            .child(rows(theme, picker));
    }

    if picker.kind() == Kind::Branches {
        return v_flex()
            .w_px(width(picker.kind()))
            .items_stretch()
            .overflow_hidden()
            .bg(theme.colors.surface)
            .border_1(theme.colors.border)
            .rounded(theme.radius.lg)
            .child(rows(theme, picker))
            .child(rule(theme))
            .child(
                field(picker.field().value(), picker.field().caret(), true)
                    .placeholder(picker.kind().placeholder())
                    .w_full()
                    .px(2)
                    .py(1.5)
                    .on_press(Message::PlacePicker),
            );
    }

    v_flex()
        .w_px(width(picker.kind()))
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
    if picker.kind() == Kind::Branches {
        return branch_rows(theme, picker);
    }
    let visible = visible_rows(picker.kind());
    let first = picker.selected().saturating_sub(visible - 1);
    let shown = picker
        .shown()
        .skip(first)
        .take(visible)
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

/// Builds grouped local and remote branches, plus the branch being typed.
fn branch_rows(theme: &Theme, picker: &Picker) -> Div<Message> {
    let query = picker.field().value().trim();
    let base = picker
        .rows()
        .find_map(|row| row.label.strip_prefix("✓  "))
        .unwrap_or("current branch");
    let mut previous = None;
    let mut built = Vec::new();

    if !query.is_empty() {
        built.push(
            v_flex()
                .w_full()
                .px(1)
                .py(0.5)
                .rounded(theme.radius.md)
                .bg(theme.colors.surface_selected)
                .hover_bg(theme.colors.surface_hover)
                .on_click(Message::CreateTypedBranch)
                .child(text(format!("＋  Create Branch: \"{query}\"…")).text_sm())
                .child(
                    text(format!("Based off {base}"))
                        .text_xs()
                        .font_light()
                        .color(theme.colors.text_subtle)
                        .pl(3),
                ),
        );
    }

    for (place, row) in picker.shown().take(BRANCH_VISIBLE) {
        if row.section != previous {
            previous = row.section;
            if let Some(section) = row.section {
                built.push(
                    h_flex().w_full().px(1.5).pt(1).pb(0.5).child(
                        text(section)
                            .text_xs()
                            .font_light()
                            .color(theme.colors.text_subtle),
                    ),
                );
            }
        }
        let color = if row.enabled {
            theme.colors.text
        } else {
            theme.colors.text_subtle
        };
        built.push(
            v_flex()
                .w_full()
                .px(1.5)
                .py(0.5)
                .overflow_hidden()
                .when(place == picker.selected() && query.is_empty(), |line| {
                    line.bg(theme.colors.surface_selected)
                })
                .hover_bg(theme.colors.surface_hover)
                .on_click(Message::ChoosePicker(place))
                .child(text(row.label.clone()).text_sm().color(color))
                .when(!row.detail.is_empty(), |line| {
                    line.child(
                        text(row.detail.clone())
                            .text_xs()
                            .font_light()
                            .color(theme.colors.text_subtle)
                            .pl(3),
                    )
                }),
        );
    }

    v_flex().w_full().py(0.5).items_stretch().children(built)
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
        .when(selected || row.detail.starts_with("current"), |line| {
            line.bg(theme.colors.surface_selected)
        })
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
        Kind::LinkedPath | Kind::CopiedPath => {
            "A path from the repository's root, like .env or web/node_modules"
        }
        Kind::PortVariable => "Leave it empty to hand a session no port at all",
        Kind::ThemeColor(_) => "Leave it as it is to keep the colour the theme gives it",
        Kind::ThemeName => "Written to the editor's home, and drawn in from now on",
        Kind::KeymapName => "Written to the editor's home, and pressed from now on",
        _ => "This cannot be undone",
    };

    h_flex().w_full().px(2).pb(1.5).child(
        text(label)
            .text_xs()
            .font_light()
            .color(theme.colors.text_subtle),
    )
}
