//! The theme mode toggle.

use pm_ui::{Div, Styled, h_flex, space, text, toggle_row};

use crate::onboarding::setup::{Message, Setup, ThemeMode};

/// The theme mode toggle.
pub(super) fn theme_section(setup: &Setup) -> Div<Message> {
    let modes = [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System];
    let selected = modes.iter().position(|mode| *mode == setup.theme_mode);
    let options = modes
        .into_iter()
        .map(|mode| (mode.label().to_owned(), Message::SetThemeMode(mode)));

    h_flex()
        .w_full()
        .gap_4()
        .items_center()
        .justify_between()
        .child(text("Theme").font_medium())
        .child(toggle_row(options, selected).w_px(space(48.0)))
}
