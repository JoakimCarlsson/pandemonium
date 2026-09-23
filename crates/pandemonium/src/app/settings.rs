//! What the window does with its settings pane: opening it, and scrolling it.

use pm_ui::Theme;

use crate::app::App;
use crate::config::WorktreePaths;
use crate::panes::{Content, Item};
use crate::picker::Kind;
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
}
