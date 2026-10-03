//! The picker as it is drawn: a field over a list, centred over the window.
//!
//! One screen draws every list the window asks a reader to choose from, and
//! the two prompts that ask for a line of text instead — a prompt is the
//! same panel with nothing under the field. What fills the list is the
//! window's; how it reads is here.

use pm_ui::{
    Div, IconName, IconSize, Styled, Theme, field, h_flex, icon, kbd, rule, space, text, v_flex,
};

use crate::agent::mode_icon;
use crate::message::Message;
use crate::picker::state::{Choice, Kind, Picker, Row};

/// How wide the panel is drawn, and the command center that opens it.
pub const WIDTH: f32 = 600.0;

/// Width of the branch popover attached to the status bar.
const BRANCH_WIDTH: f32 = 360.0;
/// Width of agent control choices beside their control.
const AGENT_WIDTH: f32 = 340.0;

/// Height of one knob set in place under an agent control's choices.
const KNOB_HEIGHT: f32 = 34.0;

/// Height of the title over an agent control's choices.
const HEADING_HEIGHT: f32 = 32.0;

/// Most choices an agent control shows at once.
const AGENT_VISIBLE: usize = 8;

/// How far from the top of the window it hangs: over the command center in
/// the title bar, the way the field it stands for is drawn there.
pub const TOP: f32 = 6.0;

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

/// Width of a picker drawn beside the control that opened it, with branch
/// workflows using their compact popover size.
pub fn width(kind: Kind) -> f32 {
    match kind {
        Kind::Branches | Kind::NewBranch => BRANCH_WIDTH,
        Kind::Agents | Kind::Modes | Kind::Knob => AGENT_WIDTH,
        _ => WIDTH,
    }
}

/// Height occupied by the visible portion of `picker`.
pub fn height(theme: &Theme, picker: &Picker) -> f32 {
    if picker.kind() == Kind::Branches {
        let shown = picker.shown().take(BRANCH_VISIBLE).collect::<Vec<_>>();
        let sections = shown
            .iter()
            .filter_map(|(_, row)| row.section)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let rows = shown
            .iter()
            .map(|(_, row)| {
                theme.text.sm.line_height
                    + if row.detail.is_empty() {
                        0.0
                    } else {
                        theme.text.xs.line_height
                    }
                    + space(1.0)
            })
            .sum::<f32>();
        let section_height = theme.text.xs.line_height + space(1.5);
        let creation_height = if picker.field().value().trim().is_empty() {
            0.0
        } else {
            theme.text.sm.line_height + theme.text.xs.line_height + space(1.0)
        };
        return space(1.0)
            + rows
            + sections as f32 * section_height
            + creation_height
            + 1.0
            + theme.text.sm.line_height
            + space(3.0);
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

/// Builds the panel for `picker`, `width` wide, over whatever the window is
/// showing.
pub fn picker(theme: &Theme, picker: &Picker, width: f32, solid: bool) -> Div<Message> {
    let prompt = picker.kind().is_prompt();

    if picker.kind() == Kind::Branches {
        return v_flex()
            .w_px(width)
            .items_stretch()
            .overflow_hidden()
            .bg(theme.colors.surface)
            .border_1(theme.colors.border)
            .rounded(theme.radius.lg)
            .child(rows(theme, picker))
            .child(rule(theme))
            .child(
                field(picker.field().value(), picker.field().caret(), solid)
                    .selection(picker.field().selection())
                    .placeholder(picker.kind().placeholder())
                    .w_full()
                    .px(2)
                    .py(1.5)
                    .on_press(Message::PlacePicker),
            );
    }

    v_flex()
        .w_px(width)
        .items_stretch()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .child(
            field(picker.field().value(), picker.field().caret(), solid)
                .selection(picker.field().selection())
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

/// Height of an agent control's choices with `knobs` knobs set beneath them.
pub fn agent_height(theme: &Theme, picker: &Picker, knobs: usize) -> f32 {
    let rows = agent_shown(picker)
        .iter()
        .map(|(_, row)| agent_row_height(theme, row))
        .sum::<f32>();
    let beneath = match knobs {
        0 => 0.0,
        knobs => 1.0 + space(1.0) + knobs as f32 * KNOB_HEIGHT,
    };
    HEADING_HEIGHT + rows + space(0.5) + beneath + 2.0
}

/// Builds an agent control's choices: `title` over them with `keys` that
/// step through them, a row of name and description for each, the current
/// one checked, and `knobs` set in place beneath them.
pub fn agent_choices(
    theme: &Theme,
    picker: &Picker,
    width: f32,
    title: &str,
    keys: Option<String>,
    knobs: Vec<Div<Message>>,
) -> Div<Message> {
    let modes = picker.kind() == Kind::Modes;
    let rows = agent_shown(picker)
        .into_iter()
        .map(|(place, row)| agent_row(theme, place, row, place == picker.selected(), modes))
        .collect::<Vec<_>>();
    let set = !knobs.is_empty();

    v_flex()
        .w_px(width)
        .items_stretch()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .child(
            h_flex()
                .w_full()
                .h_px(HEADING_HEIGHT)
                .px(1.5)
                .gap(0.5)
                .items_center()
                .child(
                    text(title.to_owned())
                        .text_xs()
                        .color(theme.colors.text_subtle),
                )
                .child(h_flex().flex_1())
                .when_some(keys, |heading, keys| {
                    heading
                        .child(kbd(theme, keys))
                        .child(text("to switch").text_xs().color(theme.colors.text_subtle))
                }),
        )
        .child(
            v_flex()
                .w_full()
                .px(0.5)
                .pb(0.5)
                .items_stretch()
                .children(rows),
        )
        .when(set, |panel| {
            panel.child(rule(theme)).child(
                v_flex()
                    .w_full()
                    .px(1.5)
                    .py(0.5)
                    .items_stretch()
                    .children(knobs.into_iter().map(|knob| {
                        h_flex()
                            .w_full()
                            .h_px(KNOB_HEIGHT)
                            .items_center()
                            .child(knob)
                    })),
            )
        })
}

/// The choices an agent control shows: a window of them around the selected one.
fn agent_shown(picker: &Picker) -> Vec<(usize, &Row)> {
    let first = picker.selected().saturating_sub(AGENT_VISIBLE - 1);
    picker.shown().skip(first).take(AGENT_VISIBLE).collect()
}

/// What one agent choice says under its name, and whether it is the current one.
///
/// The rows carry the current choice as a lead on their detail, the way a
/// wider picker shows it in words; here it is a check instead.
fn described(row: &Row) -> (bool, &str) {
    match row.detail.strip_prefix("current") {
        Some(rest) => (true, rest.trim_start_matches(" · ")),
        None => (false, row.detail.as_str()),
    }
}

/// Height of one agent choice: its name, and its description when it has one.
fn agent_row_height(theme: &Theme, row: &Row) -> f32 {
    let description = match described(row).1.is_empty() {
        true => 0.0,
        false => theme.text.xs.line_height,
    };
    theme.text.sm.line_height + description + space(1.0)
}

/// Builds one agent choice, lit while it is selected and checked while it is
/// current, with its mode's icon before it when it is a mode.
fn agent_row(theme: &Theme, place: usize, row: &Row, selected: bool, modes: bool) -> Div<Message> {
    let (current, description) = described(row);
    let mode = match &row.choice {
        Choice::Mode(_, id) if modes => mode_icon(id),
        _ => None,
    };

    h_flex()
        .w_full()
        .h_px(agent_row_height(theme, row))
        .px(1)
        .gap(1)
        .items_center()
        .rounded(theme.radius.md)
        .when(selected, |line| line.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ChoosePicker(place))
        .when(modes, |line| {
            line.child(
                h_flex()
                    .size_px(16.0)
                    .items_center()
                    .justify_center()
                    .when_some(mode, |slot, name| {
                        slot.child(
                            icon(name)
                                .size(IconSize::Medium)
                                .color(theme.colors.text_muted),
                        )
                    }),
            )
        })
        .child(
            v_flex()
                .flex_1()
                .overflow_hidden()
                .child(text(row.label.clone()).text_sm().color(theme.colors.text))
                .when(!description.is_empty(), |line| {
                    line.child(
                        text(description.to_owned())
                            .text_xs()
                            .color(theme.colors.text_subtle),
                    )
                }),
        )
        .when(current, |line| {
            line.child(
                icon(IconName::Check)
                    .size(IconSize::Medium)
                    .color(theme.colors.text),
            )
        })
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
                .when(
                    matches!(row.choice, crate::picker::Choice::Session(..)),
                    |line| {
                        if let crate::picker::Choice::Session(_, health) = row.choice {
                            line.child(health.badge(theme, &row.detail))
                        } else {
                            line
                        }
                    },
                )
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
        .child(match row.choice {
            crate::picker::Choice::Session(_, health) => health.badge(theme, &row.detail),
            _ => h_flex(),
        })
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
        Kind::BreakpointCondition | Kind::BreakpointHits => {
            "Passed to the adapter as typed, e.g. 5 or >= 5"
        }
        Kind::BreakpointLog => "Use {expression} to interpolate a value",
        Kind::Watch => "Evaluated each time the program pauses",
        Kind::LinkedPath | Kind::CopiedPath => {
            "A path from the repository's root, like .env or web/node_modules"
        }
        Kind::PortVariable => "Leave it empty to hand a session no port at all",
        Kind::ThemeColor(_) => "Leave it as it is to keep the colour the theme gives it",
        Kind::ThemeName => "Written to the editor's home, and drawn in from now on",
        Kind::KeymapName => "Written to the editor's home, and pressed from now on",
        Kind::LanguageFormatter(_) => {
            "Runs in the file's folder; {path} stands for the file's path"
        }
        _ => "This cannot be undone",
    };

    h_flex().w_full().px(2).pb(1.5).child(
        text(label)
            .text_xs()
            .font_light()
            .color(theme.colors.text_subtle),
    )
}
