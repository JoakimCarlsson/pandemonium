//! The shape the preferences take on disk.
//!
//! Distinct from the in-memory state so the file survives that shape changing:
//! the theme family is stored by name rather than by its index into
//! [`FAMILIES`], and every field is optional so an older file still loads.

use std::path::PathBuf;

use pm_ui::FAMILIES;
use serde::{Deserialize, Serialize};

use crate::config::Restored;
use crate::keymap::BaseKeymap;
use crate::onboarding::{Setup, ThemeMode};

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
}

impl Stored {
    /// What this file stands for, defaulting anything it leaves out.
    pub(super) fn into_restored(self) -> Restored {
        let projects = self.projects.clone().unwrap_or_default();
        Restored {
            setup: self.into_setup(),
            projects,
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
    /// The file to write for these preferences and these open projects.
    pub(super) fn of(setup: &Setup, projects: &[PathBuf]) -> Self {
        Self {
            theme_mode: Some(setup.theme_mode),
            theme_family: Some(pm_ui::family(setup.theme_family).name.to_owned()),
            keymap: Some(setup.keymap),
            vim_mode: Some(setup.vim_mode),
            trust_worktrees: Some(setup.trust_worktrees),
            metrics: Some(setup.metrics),
            crash_reports: Some(setup.crash_reports),
            finished: Some(setup.finished),
            projects: Some(projects.to_vec()),
        }
    }
}

/// The index into [`FAMILIES`] of the family called `name`.
fn family_index(name: &str) -> Option<usize> {
    FAMILIES.iter().position(|family| family.name == name)
}
