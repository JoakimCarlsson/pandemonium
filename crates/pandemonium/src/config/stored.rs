//! The shape the preferences take on disk.
//!
//! Distinct from the in-memory state so the file survives that shape changing:
//! the theme family is stored by name rather than by its index into
//! [`FAMILIES`], and every field is optional so an older file still loads.

use std::path::PathBuf;

use pm_ui::FAMILIES;
use serde::{Deserialize, Serialize};

use crate::config::{Restored, WindowState};
use crate::keymap::BaseKeymap;
use crate::onboarding::{Setup, ThemeMode};
use crate::workspace::Layout;

/// The preferences as they are written down.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct Stored {
    /// Which theme the editor draws in.
    theme_mode: Option<ThemeMode>,
    /// The name of the theme family the editor draws in.
    theme_family: Option<String>,
    /// The keymap the editor starts from.
    keymap: Option<BaseKeymap>,
    /// Whether editing starts in vim mode.
    vim_mode: Option<bool>,
    /// Whether a new session's worktree is trusted without being asked about.
    trust_worktrees: Option<bool>,
    /// Whether anonymous usage data is sent.
    metrics: Option<bool>,
    /// Whether crash reports are sent.
    crash_reports: Option<bool>,
    /// Whether setup has been finished, which swaps the page.
    finished: Option<bool>,
    /// The roots of the projects the window had open.
    projects: Option<Vec<PathBuf>>,
    /// The root of the project the window was pointed at.
    active_project: Option<PathBuf>,
    /// Whether the primary sidebar was visible.
    primary_sidebar_open: Option<bool>,
    /// Width of the primary sidebar.
    primary_sidebar_width: Option<f32>,
    /// Whether the bottom panel was visible.
    bottom_panel_open: Option<bool>,
    /// Height of the bottom panel.
    bottom_panel_height: Option<f32>,
    /// Whether the secondary sidebar was visible.
    secondary_sidebar_open: Option<bool>,
    /// Width of the secondary sidebar.
    secondary_sidebar_width: Option<f32>,
    /// Logical width of the window when it is not maximized.
    window_width: Option<f32>,
    /// Logical height of the window when it is not maximized.
    window_height: Option<f32>,
    /// Whether the window filled the screen it was on.
    window_maximized: Option<bool>,
}

impl Stored {
    /// What this file stands for, defaulting anything it leaves out.
    pub(super) fn into_restored(self) -> Restored {
        Restored {
            projects: self.projects.clone().unwrap_or_default(),
            active: self.active_project.clone(),
            layout: self.layout(),
            window: self.window(),
            setup: self.into_setup(),
        }
    }

    /// The regions this file stands for, defaulting anything it leaves out.
    fn layout(&self) -> Layout {
        let defaults = Layout::default();
        Layout {
            primary_sidebar_open: self
                .primary_sidebar_open
                .unwrap_or(defaults.primary_sidebar_open),
            primary_sidebar_width: self
                .primary_sidebar_width
                .unwrap_or(defaults.primary_sidebar_width),
            bottom_panel_open: self.bottom_panel_open.unwrap_or(defaults.bottom_panel_open),
            bottom_panel_height: self
                .bottom_panel_height
                .unwrap_or(defaults.bottom_panel_height),
            secondary_sidebar_open: self
                .secondary_sidebar_open
                .unwrap_or(defaults.secondary_sidebar_open),
            secondary_sidebar_width: self
                .secondary_sidebar_width
                .unwrap_or(defaults.secondary_sidebar_width),
        }
    }

    /// The window this file stands for, defaulting anything it leaves out.
    fn window(&self) -> WindowState {
        let defaults = WindowState::default();
        WindowState {
            width: self.window_width.unwrap_or(defaults.width),
            height: self.window_height.unwrap_or(defaults.height),
            maximized: self.window_maximized.unwrap_or(defaults.maximized),
        }
    }

    /// The preferences this file stands for, defaulting anything it leaves out.
    fn into_setup(self) -> Setup {
        let defaults = Setup::default();
        Setup {
            theme_mode: self.theme_mode.unwrap_or(defaults.theme_mode),
            theme_family: self
                .theme_family
                .as_deref()
                .and_then(family_index)
                .unwrap_or(defaults.theme_family),
            keymap: self.keymap.unwrap_or(defaults.keymap),
            vim_mode: self.vim_mode.unwrap_or(defaults.vim_mode),
            trust_worktrees: self.trust_worktrees.unwrap_or(defaults.trust_worktrees),
            metrics: self.metrics.unwrap_or(defaults.metrics),
            crash_reports: self.crash_reports.unwrap_or(defaults.crash_reports),
            finished: self.finished.unwrap_or(defaults.finished),
        }
    }
}

impl Stored {
    /// The file to write for the window as it stands.
    pub(super) fn of(restored: &Restored) -> Self {
        let Restored {
            setup,
            projects,
            active,
            layout,
            window,
        } = restored;

        Self {
            theme_mode: Some(setup.theme_mode),
            theme_family: Some(pm_ui::family(setup.theme_family).name.to_owned()),
            keymap: Some(setup.keymap),
            vim_mode: Some(setup.vim_mode),
            trust_worktrees: Some(setup.trust_worktrees),
            metrics: Some(setup.metrics),
            crash_reports: Some(setup.crash_reports),
            finished: Some(setup.finished),
            projects: Some(projects.clone()),
            active_project: active.clone(),
            primary_sidebar_open: Some(layout.primary_sidebar_open),
            primary_sidebar_width: Some(layout.primary_sidebar_width),
            bottom_panel_open: Some(layout.bottom_panel_open),
            bottom_panel_height: Some(layout.bottom_panel_height),
            secondary_sidebar_open: Some(layout.secondary_sidebar_open),
            secondary_sidebar_width: Some(layout.secondary_sidebar_width),
            window_width: Some(window.width),
            window_height: Some(window.height),
            window_maximized: Some(window.maximized),
        }
    }
}

/// The index into [`FAMILIES`] of the family called `name`.
fn family_index(name: &str) -> Option<usize> {
    FAMILIES.iter().position(|family| family.name == name)
}
