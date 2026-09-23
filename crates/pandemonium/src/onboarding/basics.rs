//! The settings themselves, in the order a first launch wants them.

use pm_ui::{
    Div, Styled, Theme, h_flex, section, space, switch_field, text, theme_gallery, toggle_grid,
    toggle_row, v_flex,
};

use crate::config::{Preference, Preferences, ThemeMode};
use crate::keymap::BaseKeymap;
use crate::message::Message;

/// The settings themselves, in the order a first launch wants them.
pub(super) fn basics(theme: &Theme, preferences: &Preferences) -> Div<Message> {
    v_flex()
        .w_full()
        .gap(6)
        .child(theme_section(theme, preferences))
        .child(keymap_section(theme, preferences))
        .child(switch_field(
            theme,
            Some("Format on Save"),
            "Lay a file out the way its formatter would every time it is written",
            preferences.format_on_save,
            Message::TogglePreference(Preference::FormatOnSave),
        ))
        .child(switch_field(
            theme,
            Some("Trust New Worktrees"),
            "Run language servers and tasks in a session's worktree without asking first",
            preferences.trust_worktrees,
            Message::TogglePreference(Preference::TrustWorktrees),
        ))
}

/// The appearance toggle over a row of theme previews.
fn theme_section(theme: &Theme, preferences: &Preferences) -> Div<Message> {
    let selected = ThemeMode::ALL
        .iter()
        .position(|mode| *mode == preferences.theme_mode);
    let options = ThemeMode::ALL
        .into_iter()
        .map(|mode| (mode.label().to_owned(), Message::SetThemeMode(mode)));

    let shown = match preferences.theme_mode {
        ThemeMode::System => None,
        _ => Some(theme.appearance),
    };

    v_flex()
        .w_full()
        .gap(3)
        .child(
            h_flex()
                .w_full()
                .gap(4)
                .items_center()
                .justify_between()
                .child(text("Theme").font_medium())
                .child(toggle_row(options, selected).w_px(space(48.0))),
        )
        .child(theme_gallery(
            shown,
            preferences.theme_family,
            Message::SetThemeFamily,
        ))
}

/// The keymap grid.
fn keymap_section(theme: &Theme, preferences: &Preferences) -> Div<Message> {
    let options = BaseKeymap::ALL
        .into_iter()
        .map(|keymap| (keymap.label().to_owned(), Message::SetKeymap(keymap)));

    section(
        theme,
        "Base Keymap",
        Some("Keep the bindings your hands already know"),
        toggle_grid(options, Some(preferences.keymap.index()), 4),
    )
}
