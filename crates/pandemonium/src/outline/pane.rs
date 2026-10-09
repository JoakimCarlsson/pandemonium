//! The outline pane's filter, source label, and indented symbol rows.

use pm_core::Scope;
use pm_ui::{Div, IconName, IconSize, Styled, Theme, h_flex, icon, text, v_flex};

use crate::input::hinted_input_view;
use crate::message::Message;
use crate::outline::Outline;

/// Height of one symbol row in logical pixels.
pub const ROW_HEIGHT: f32 = 26.0;

/// Builds a worktree's outline beside the file panes.
pub fn outline_pane(
    theme: &Theme,
    scope: Scope,
    outline: Option<&Outline>,
    focused: bool,
) -> Div<Message> {
    let Some(outline) = outline else {
        return v_flex()
            .w_full()
            .h_full()
            .bg(theme.colors.background)
            .child(text("No file").text_sm().color(theme.colors.text_muted));
    };
    let rows = outline.visible();
    let source = if outline.source.is_empty() {
        "from syntax".to_owned()
    } else {
        format!("from {}", outline.source)
    };
    let mut body = v_flex().w_full().flex_1().overflow_hidden();
    for (index, matched) in rows.into_iter().skip(outline.scroll).take(200) {
        let symbol = &outline.symbols[index];
        let has_children = outline.has_children(index);
        let chevron = if outline.is_collapsed(index) {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };
        let mut row = h_flex()
            .w_full()
            .h_px(ROW_HEIGHT)
            .items_center()
            .gap(0.5)
            .pl(0.5 + symbol.depth as f64)
            .when(outline.selected == Some(index), |row| {
                row.bg(theme.colors.surface)
            })
            .on_click(Message::OutlineSelect(scope, index));
        if has_children {
            row = row.child(
                h_flex()
                    .w_px(12.0)
                    .child(
                        icon(chevron)
                            .size(IconSize::XSmall)
                            .color(theme.colors.text_muted),
                    )
                    .on_click(Message::OutlineToggle(scope, index)),
            );
        } else {
            row = row.child(text(" ").w_px(12.0));
        }
        row = row
            .child(text(symbol.kind).text_xs().color(theme.colors.text_muted))
            .child(text(&symbol.name).text_sm().color(if matched {
                theme.colors.text
            } else {
                theme.colors.text_muted
            }))
            .child(
                text(&symbol.detail)
                    .text_xs()
                    .color(theme.colors.text_muted),
            );
        body = body.child(row);
    }
    v_flex()
        .w_full()
        .h_full()
        .bg(theme.colors.background)
        .child(
            v_flex()
                .w_full()
                .px(1)
                .py(0.5)
                .gap(0.5)
                .bg(theme.colors.surface)
                .child(
                    hinted_input_view(
                        theme,
                        &outline.filter,
                        focused,
                        focused,
                        "Filter symbols",
                        move |phase, anchor, head| {
                            Message::WriteOutlineFilter(scope, phase, anchor, head)
                        },
                        Message::ShowInputMenu,
                    )
                    .h_px(28.0),
                )
                .child(text(source).text_xs().color(theme.colors.text_muted)),
        )
        .child(body)
}
