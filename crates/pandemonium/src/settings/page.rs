//! The settings pane: a sidebar of pages, and the page it has open.
//!
//! Every row is the same shape, after Zed's: the name and what it does on
//! the left, the control on the right — or under them, for a control that
//! needs the width — and an undo mark beside the name while the value is
//! not the one a first launch starts from.

use std::path::PathBuf;

use pm_ui::{
    Div, Element, IconName, IconSize, IntoElement, Styled, Theme, button, h_flex, icon,
    icon_button, rule, scroll_area, space, switch, text, theme_gallery, toggle_grid, toggle_row,
    v_flex,
};

use crate::config::{Preference, Preferences, ThemeMode, WorktreePaths};
use crate::keymap::BaseKeymap;
use crate::message::Message;
use crate::settings::state::{Settings, SettingsPage};
use crate::workspace::shortened;

/// Width of the sidebar listing the pages.
const SIDEBAR_WIDTH: f32 = 226.0;

/// Width a page is capped at, however wide the pane is.
const PAGE_WIDTH: f32 = 760.0;

/// Width of the theme mode toggle.
const MODE_WIDTH: f32 = 48.0;

/// Width of the button naming the port variable.
const PORT_WIDTH: f32 = 32.0;

/// Diameter of the dot marking a page with something set on it.
const DOT_SIZE: f32 = 6.0;

/// What the settings pane is drawn from.
pub struct SettingsPane<'a> {
    /// Which page is open, and how far down it.
    pub settings: &'a Settings,
    /// The preferences the pane edits.
    pub preferences: &'a Preferences,
    /// The file the preferences are written to, when there is one.
    pub file: Option<PathBuf>,
}

/// Builds the settings pane in `theme`.
pub fn settings_pane(theme: &Theme, pane: &SettingsPane<'_>) -> Box<dyn Element<Message>> {
    let page = pane.settings.page();
    Box::new(
        h_flex()
            .w_full()
            .flex_1()
            .items_stretch()
            .child(sidebar(theme, pane.preferences, page))
            .child(v_flex().w_px(1.0).h_full().bg(theme.colors.border_variant))
            .child(
                scroll_area(
                    pane.settings.scroll(),
                    v_flex()
                        .w_full()
                        .max_w_px(PAGE_WIDTH)
                        .mx_auto()
                        .px(8)
                        .py(6)
                        .gap(4)
                        .child(heading(theme, page, pane.file.as_ref()))
                        .children(sections(theme, pane, page)),
                )
                .flex_1()
                .h_full(),
            ),
    )
}

/// The list of pages down the left of the pane.
fn sidebar(theme: &Theme, preferences: &Preferences, open: SettingsPage) -> Div<Message> {
    v_flex()
        .w_px(SIDEBAR_WIDTH)
        .h_full()
        .p(2)
        .gap(0.5)
        .bg(theme.colors.surface)
        .child(
            h_flex().h_px(theme.size.row).px(2).items_center().child(
                text("Settings")
                    .text_sm()
                    .font_semibold()
                    .color(theme.colors.text_muted),
            ),
        )
        .children(
            SettingsPage::ALL
                .into_iter()
                .map(|page| page_entry(theme, preferences, page, page == open)),
        )
}

/// One page in the sidebar, marked when something on it is set.
fn page_entry(
    theme: &Theme,
    preferences: &Preferences,
    page: SettingsPage,
    open: bool,
) -> Div<Message> {
    let modified = page
        .preferences()
        .iter()
        .any(|preference| preferences.is_modified(*preference));
    let color = if open {
        theme.colors.text
    } else {
        theme.colors.text_muted
    };

    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(2)
        .gap(2)
        .items_center()
        .justify_between()
        .rounded(theme.radius.md)
        .when(open, |entry| entry.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ShowSettingsPage(page))
        .child(text(page.label()).text_sm().color(color))
        .when(modified, |entry| {
            entry.child(
                v_flex()
                    .size_px(DOT_SIZE)
                    .rounded(DOT_SIZE / 2.0)
                    .bg(theme.colors.text_muted),
            )
        })
}

/// The page's name, and where what it sets is written down.
fn heading(theme: &Theme, page: SettingsPage, file: Option<&PathBuf>) -> Div<Message> {
    v_flex()
        .w_full()
        .gap(1)
        .child(text(page.label()).text_xl().font_semibold())
        .when_some(file, |heading, file| {
            heading.child(
                text(format!("Written to {}", shortened(file)))
                    .text_xs()
                    .font_mono()
                    .color(theme.colors.text_subtle),
            )
        })
}

/// The sections of `page`, each a heading over its rows.
fn sections(theme: &Theme, pane: &SettingsPane<'_>, page: SettingsPage) -> Vec<Div<Message>> {
    let preferences = pane.preferences;
    match page {
        SettingsPage::Appearance => vec![section(
            theme,
            "Theme",
            vec![
                inline(
                    theme,
                    preferences,
                    Preference::ThemeMode,
                    "Mode",
                    "Keep to one appearance, or follow the desktop's",
                    theme_modes(preferences).w_px(space(MODE_WIDTH)),
                ),
                below(
                    theme,
                    preferences,
                    Preference::ThemeFamily,
                    "Family",
                    "The family the editor is painted in; themes in the editor's home are listed too",
                    theme_gallery(
                        shown_appearance(theme, preferences),
                        preferences.theme_family,
                        Message::SetThemeFamily,
                    ),
                ),
            ],
        )],
        SettingsPage::Keymap => vec![section(
            theme,
            "Bindings",
            vec![
                below(
                    theme,
                    preferences,
                    Preference::Keymap,
                    "Base Keymap",
                    "Keep the bindings your hands already know",
                    toggle_grid(keymaps(), Some(preferences.keymap.index()), 4),
                ),
                toggle(
                    theme,
                    preferences,
                    Preference::VimMode,
                    "Vim Mode",
                    "Modal editing, built in rather than an extension",
                    preferences.vim_mode,
                    Message::ToggleVimMode,
                ),
            ],
        )],
        SettingsPage::Editor => vec![section(
            theme,
            "Saving",
            vec![toggle(
                theme,
                preferences,
                Preference::FormatOnSave,
                "Format on Save",
                "Lay a file out the way its formatter would every time it is written",
                preferences.format_on_save,
                Message::ToggleFormatOnSave,
            )],
        )],
        SettingsPage::Sessions => vec![
            section(
                theme,
                "Trust",
                vec![toggle(
                    theme,
                    preferences,
                    Preference::TrustWorktrees,
                    "Trust New Worktrees",
                    "Run language servers and tasks in a session's worktree without asking first",
                    preferences.trust_worktrees,
                    Message::ToggleTrustWorktrees,
                )],
            ),
            section(theme, "New Worktrees", bootstrap_rows(theme, preferences)),
        ],
    }
}

/// What a fresh worktree is given: the paths linked and copied into it,
/// and the variable its port is handed in.
fn bootstrap_rows(theme: &Theme, preferences: &Preferences) -> Vec<Div<Message>> {
    vec![
        below(
            theme,
            preferences,
            Preference::WorktreeLink,
            "Linked In",
            "Symlinked from the repository into every new worktree, so they share one copy",
            path_list(theme, preferences, WorktreePaths::Linked),
        ),
        below(
            theme,
            preferences,
            Preference::WorktreeCopy,
            "Copied In",
            "Copied from the repository into every new worktree, so each can change its own",
            path_list(theme, preferences, WorktreePaths::Copied),
        ),
        inline(
            theme,
            preferences,
            Preference::WorktreePort,
            "Port Variable",
            "The variable a session's own port is handed to its programs in",
            button(
                preferences
                    .bootstrap
                    .port
                    .clone()
                    .unwrap_or_else(|| "None".to_owned()),
                Message::EditWorktreePort,
            )
            .outlined()
            .w_px(space(PORT_WIDTH)),
        ),
    ]
}

/// One list of paths a new worktree is given, each with a way off it, and
/// a way to add another.
fn path_list(theme: &Theme, preferences: &Preferences, list: WorktreePaths) -> Div<Message> {
    let paths = preferences.worktree_paths(list);
    let entries = paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            h_flex()
                .w_full()
                .h_px(theme.size.row)
                .pl(2)
                .pr(1)
                .items_center()
                .justify_between()
                .rounded(theme.radius.md)
                .bg(theme.colors.surface)
                .child(text(path.display().to_string()).text_sm().font_mono())
                .child(
                    icon_button(
                        theme,
                        IconName::Close,
                        Message::RemoveWorktreePath(list, index),
                    )
                    .tooltip("Remove"),
                )
        })
        .collect::<Vec<_>>();

    v_flex()
        .w_full()
        .gap(1)
        .when(paths.is_empty(), |list| {
            list.child(
                text("Nothing yet")
                    .text_sm()
                    .color(theme.colors.text_subtle),
            )
        })
        .children(entries)
        .child(
            h_flex()
                .w_fit()
                .h_px(theme.size.row)
                .px(2)
                .gap(1.5)
                .items_center()
                .rounded(theme.radius.md)
                .hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(Message::AddWorktreePath(list))
                .child(
                    icon(IconName::Plus)
                        .size(IconSize::Small)
                        .color(theme.colors.text_muted),
                )
                .child(text("Add Path").text_sm().color(theme.colors.text_muted)),
        )
}

/// A heading over rows, with a hairline between each row and the next.
fn section(theme: &Theme, title: &str, rows: Vec<Div<Message>>) -> Div<Message> {
    let mut body = v_flex().w_full();
    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 {
            body = body.child(rule(theme));
        }
        body = body.child(row);
    }

    v_flex()
        .w_full()
        .gap(1)
        .child(
            text(title)
                .text_xs()
                .font_semibold()
                .color(theme.colors.text_subtle),
        )
        .child(body)
}

/// A row with a switch on the right.
fn toggle(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
    title: &str,
    description: &str,
    on: bool,
    message: Message,
) -> Div<Message> {
    inline(
        theme,
        preferences,
        preference,
        title,
        description,
        switch(on, message),
    )
}

/// A row with its control on the right of its labels.
fn inline(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
    title: &str,
    description: &str,
    control: impl IntoElement<Message>,
) -> Div<Message> {
    h_flex()
        .w_full()
        .py(3)
        .gap(6)
        .items_center()
        .justify_between()
        .child(labels(theme, preferences, preference, title, description).flex_1())
        .child(control)
}

/// A row with its control under its labels, across the whole row.
fn below(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
    title: &str,
    description: &str,
    control: impl IntoElement<Message>,
) -> Div<Message> {
    v_flex()
        .w_full()
        .py(3)
        .gap(3)
        .child(labels(theme, preferences, preference, title, description))
        .child(control)
}

/// A row's name, the undo mark while it is set, and what it does.
fn labels(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
    title: &str,
    description: &str,
) -> Div<Message> {
    let reset = preferences.is_modified(preference).then(|| {
        icon_button(theme, IconName::Undo, Message::ResetPreference(preference))
            .tooltip("Reset to Default")
    });

    v_flex()
        .gap(0.5)
        .child(
            h_flex()
                .h_px(theme.size.icon_control)
                .gap(1)
                .items_center()
                .child(text(title).font_medium())
                .when_some(reset, Div::child),
        )
        .child(text(description).text_sm().color(theme.colors.text_muted))
}

/// The theme modes, as a toggle with the chosen one lit.
fn theme_modes(preferences: &Preferences) -> Div<Message> {
    let selected = ThemeMode::ALL
        .iter()
        .position(|mode| *mode == preferences.theme_mode);
    toggle_row(
        ThemeMode::ALL
            .into_iter()
            .map(|mode| (mode.label().to_owned(), Message::SetThemeMode(mode))),
        selected,
    )
}

/// The appearance the theme previews are painted in, or none for both.
fn shown_appearance(theme: &Theme, preferences: &Preferences) -> Option<pm_ui::Appearance> {
    match preferences.theme_mode {
        ThemeMode::System => None,
        _ => Some(theme.appearance),
    }
}

/// Every keymap, with the message that picks it.
fn keymaps() -> impl Iterator<Item = (String, Message)> {
    BaseKeymap::ALL
        .into_iter()
        .map(|keymap| (keymap.label().to_owned(), Message::SetKeymap(keymap)))
}
