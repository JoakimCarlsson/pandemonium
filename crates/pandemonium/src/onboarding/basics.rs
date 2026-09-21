//! The settings themselves, in the order a first launch wants them.

use pm_ui::{
    Div, FAMILIES, Styled, Theme, h_flex, rule, section, space, switch_field, text, theme_preview,
    toggle_grid, toggle_row, v_flex,
};

use crate::keymap::BaseKeymap;
use crate::onboarding::setup::{Message, Setup, ThemeMode};

/// The settings themselves, in the order a first launch wants them.
pub(super) fn basics(theme: &Theme, setup: &Setup) -> Div<Message> {
    v_flex()
        .w_full()
        .gap_6()
        .child(theme_section(theme, setup))
        .child(keymap_section(theme, setup))
        .child(switch_field(
            theme,
            Some("Vim Mode"),
            "Coming from vim? Modal editing is built in, not an extension",
            setup.vim_mode,
            Message::ToggleVimMode,
        ))
        .child(switch_field(
            theme,
            Some("Trust New Worktrees"),
            "Run language servers and tasks in a session's worktree without asking first",
            setup.trust_worktrees,
            Message::ToggleTrustWorktrees,
        ))
        .child(rule(theme))
        .child(switch_field(
            theme,
            None,
            "Help improve Pandemonium by sending anonymous usage data",
            setup.metrics,
            Message::ToggleMetrics,
        ))
        .child(switch_field(
            theme,
            None,
            "Send crash reports so the crashes you hit get fixed",
            setup.crash_reports,
            Message::ToggleCrashReports,
        ))
}

/// The appearance toggle over a row of theme previews.
fn theme_section(theme: &Theme, setup: &Setup) -> Div<Message> {
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

/// The keymap grid.
fn keymap_section(theme: &Theme, setup: &Setup) -> Div<Message> {
    let options = BaseKeymap::ALL
        .into_iter()
        .map(|keymap| (keymap.label().to_owned(), Message::SetKeymap(keymap)));

    section(
        theme,
        "Base Keymap",
        Some("Keep the bindings your hands already know"),
        toggle_grid(options, Some(setup.keymap.index()), 4),
    )
}
