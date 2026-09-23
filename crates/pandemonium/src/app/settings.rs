//! What the window does with its settings pane: opening it, scrolling it,
//! asking for what a row cannot take by itself, and handing what changed to
//! the parts of the window that act on it.

use pm_ui::Theme;

use crate::app::App;
use crate::config::{self, FontSlot, TOKENS, WorktreePaths};
use crate::editor::Habits;
use crate::message::Message;
use crate::panes::{Content, Item};
use crate::picker::{Choice, Kind, Row};
use crate::settings::{SettingsPane, settings_pane};

impl App {
    /// Brings the settings pane forward, opening it in the pane with the
    /// keyboard when no pane has it open.
    ///
    /// There is one settings pane for the window, as there is one file of
    /// preferences: asking for it again finds the tab it already has rather
    /// than opening a second. Until the first run's setup is finished the
    /// setup page is the settings, and there is nothing to open.
    pub(super) fn open_settings(&mut self) {
        if !self.onboarded {
            return;
        }
        let scope = self.scope();
        let holding = self
            .panes
            .panes()
            .into_iter()
            .find(|pane| {
                self.panes
                    .pane(*pane)
                    .is_some_and(|pane| pane.items().any(|item| item == Item::Settings))
            })
            .unwrap_or_else(|| self.panes.focus());
        if let Some(pane) = self.panes.pane_mut(holding) {
            pane.open(scope, Item::Settings);
        }
        self.focus_pane(holding);
        self.store();
    }

    /// What a pane showing the settings draws beneath its bar of tabs.
    pub(super) fn settings_content(&self, theme: &Theme) -> Content {
        Content::Built(settings_pane(
            theme,
            &SettingsPane {
                settings: &self.settings,
                preferences: &self.preferences,
                file: crate::config::settings_file(),
            },
        ))
    }

    /// Asks for a path to add to the `list` a new worktree is given.
    pub(super) fn ask_worktree_path(&mut self, list: WorktreePaths) {
        let kind = match list {
            WorktreePaths::Linked => Kind::LinkedPath,
            WorktreePaths::Copied => Kind::CopiedPath,
        };
        self.open_picker_with(kind, Vec::new(), String::new());
    }

    /// Asks for the variable a session's port is handed in, starting from
    /// the one it is handed in now.
    pub(super) fn ask_worktree_port(&mut self) {
        let named = self.preferences.bootstrap.port.clone().unwrap_or_default();
        self.open_picker_with(Kind::PortVariable, Vec::new(), named);
    }

    /// Adds what was typed to the `list` a new worktree is given.
    pub(super) fn add_worktree_path(&mut self, list: WorktreePaths, typed: &str) {
        self.preferences.add_worktree_path(list, typed);
        self.store();
    }

    /// Hands a session's port in the variable that was typed.
    pub(super) fn set_worktree_port(&mut self, typed: &str) {
        self.preferences.set_worktree_port(typed);
        self.store();
    }

    /// Scrolls the settings pane by `delta` logical pixels when the pointer
    /// is over it, saying whether it was.
    pub(super) fn scroll_settings(&mut self, delta: f32) -> bool {
        let pane = self
            .pointer
            .and_then(|at| self.geometry.pane_at(at))
            .unwrap_or_else(|| self.panes.focus());
        let showing = self
            .panes
            .pane(pane)
            .and_then(|pane| pane.active(self.scope()))
            == Some(Item::Settings);
        if showing {
            self.settings.scroll_by(delta);
        }
        showing
    }

    /// Carries out a message of the settings pane that asks rather than
    /// sets, saying whether `message` was one.
    pub(super) fn settings_command(&mut self, message: Message) -> bool {
        match message {
            Message::PickFont(slot) => self.ask_font(slot),
            Message::EditThemeColor(token) => self.ask_theme_color(token),
            Message::SaveTheme => self.ask_theme_name(),
            Message::ReloadThemes => {
                config::reload_themes(&mut self.preferences);
                self.store();
            }
            _ => return false,
        }
        true
    }

    /// Hands the preferences to everything that keeps a copy of one: the
    /// open files, the shells, and the hints already written into lines.
    pub(super) fn follow_preferences(&mut self) {
        let preferences = &self.preferences;
        self.editor.set_habits(Habits {
            indent: preferences.indent(),
            trim_whitespace: preferences.trim_whitespace,
            final_newline: preferences.final_newline,
        });
        self.terminals
            .set_scrollback(preferences.terminal_scrollback);
        if !preferences.inlay_hints {
            self.editor.forget_hints();
        }
    }

    /// Asks which installed family to set `slot` in, the one it is set in
    /// now marked.
    fn ask_font(&mut self, slot: FontSlot) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let families = renderer.text().families(slot == FontSlot::Buffer);
        let current = self.preferences.fonts.family(slot);
        let row = |label: String, family: Option<String>| Row {
            section: None,
            detail: match family.as_deref() == current {
                true => "Current".to_owned(),
                false => String::new(),
            },
            label,
            choice: Choice::Font(slot, family),
            enabled: true,
        };
        let rows = std::iter::once(row("Default".to_owned(), None))
            .chain(
                families
                    .into_iter()
                    .map(|family| row(family.clone(), Some(family))),
            )
            .collect();
        self.open_picker_with(Kind::Font(slot), rows, String::new());
    }

    /// Sets `slot` in `family`, or in the editor's pick for none.
    pub(super) fn set_font(&mut self, slot: FontSlot, family: Option<String>) {
        self.preferences.fonts.set_family(slot, family);
        self.store();
    }

    /// Asks what to repaint `token` in, starting from what it is now.
    fn ask_theme_color(&mut self, token: usize) {
        let Some(color) = TOKENS.get(token).map(|found| found.read(&self.theme())) else {
            return;
        };
        self.open_picker_with(Kind::ThemeColor(token), Vec::new(), config::hex(color));
    }

    /// Repaints `token` in the colour typed, over the appearance in front.
    ///
    /// What does not read as a colour leaves the colour as it was. The
    /// colour the family itself gives it is no override at all, so typing
    /// that takes the override away rather than writing it down.
    pub(super) fn set_theme_color(&mut self, token: usize, typed: &str) {
        let (Some(color), Some(found)) = (config::from_hex(typed), TOKENS.get(token)) else {
            return;
        };
        let appearance = self.theme().appearance;
        let family = pm_ui::family(self.preferences.theme_family).variant(appearance);
        let overrides = &mut self.preferences.theme_overrides;
        match found.read(&family) == color {
            true => overrides.clear(appearance, token),
            false => overrides.set(appearance, token, color),
        }
        self.store();
    }

    /// Asks what to call the theme being drawn in, before writing it down.
    fn ask_theme_name(&mut self) {
        let named = format!(
            "{} Custom",
            pm_ui::family(self.preferences.theme_family).name
        );
        self.open_picker_with(Kind::ThemeName, Vec::new(), named);
    }

    /// Writes the theme being drawn in down as one called `typed`.
    pub(super) fn save_theme(&mut self, typed: &str) {
        if config::save_theme(&mut self.preferences, typed) {
            self.store();
        }
    }
}
