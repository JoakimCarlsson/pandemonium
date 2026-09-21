//! The theme section: the appearance toggle and one preview tile per family.

use pm_ui::{Div, FAMILIES, Styled, Theme, h_flex, space, text, theme_preview, toggle_row, v_flex};

use crate::onboarding::setup::{Message, Setup, ThemeMode};

/// The appearance toggle over a row of theme previews.
pub(super) fn theme_section(theme: &Theme, setup: &Setup) -> Div<Message> {
    let modes = [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System];
    let selected = modes.iter().position(|mode| *mode == setup.theme_mode);
    let options = modes
        .into_iter()
        .map(|mode| (mode.label().to_owned(), Message::SetThemeMode(mode)));

    let shown = match setup.theme_mode {
        ThemeMode::System => None,
        _ => Some(theme.appearance),
    };
    let previews = FAMILIES
        .iter()
        .enumerate()
        .map(|(index, family)| {
            theme_preview(
                *family,
                shown,
                index == setup.theme_family,
                Message::SetThemeFamily(index),
            )
            .flex_1()
        })
        .collect::<Vec<_>>();

    v_flex()
        .w_full()
        .gap_3()
        .child(
            h_flex()
                .w_full()
                .gap_4()
                .items_center()
                .justify_between()
                .child(text("Theme").font_medium())
                .child(toggle_row(options, selected).w_px(space(48.0))),
        )
        .child(h_flex().w_full().gap_2().children(previews))
}
