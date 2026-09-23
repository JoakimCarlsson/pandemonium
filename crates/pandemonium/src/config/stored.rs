//! The shape the preferences take on disk.
//!
//! Distinct from the in-memory state so the file survives that shape changing:
//! the theme family is stored by name rather than by its index into
//! the families on offer, and every field is optional so an older file still
//! loads.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pm_core::Bootstrap;
use pm_text::Server;
use pm_ui::families;
use serde::{Deserialize, Serialize};

use crate::config::{Preferences, Restored, ThemeMode, WindowState};
use crate::keymap::BaseKeymap;
use crate::panes::Saved;
use crate::workspace::{Layout, SidebarView};

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
    /// Whether a file is laid out the way its formatter would when it is saved.
    format_on_save: Option<bool>,
    /// Whether a new session's worktree is trusted without being asked about.
    trust_worktrees: Option<bool>,
    /// Whether anonymous usage data is sent.
    metrics: Option<bool>,
    /// Whether crash reports are sent.
    crash_reports: Option<bool>,
    /// The servers to run for a language, in place of the ones it names.
    language_servers: Option<BTreeMap<String, Vec<StoredServer>>>,
    /// Paths symlinked into a fresh worktree, relative to the repository.
    worktree_link: Option<Vec<PathBuf>>,
    /// Paths copied into it, relative to the repository.
    worktree_copy: Option<Vec<PathBuf>>,
    /// The variable a session's own port is handed to a program in.
    worktree_port: Option<String>,
    /// Whether the first run's setup has been finished.
    finished: Option<bool>,
    /// The roots of the projects the window had open.
    projects: Option<Vec<PathBuf>>,
    /// The root of the project the window was pointed at.
    active_project: Option<PathBuf>,
    /// How the window was divided into panes, and what was open in them.
    panes: Option<Saved>,
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
    /// Which of the worktree's two lists that sidebar was showing.
    secondary_sidebar_view: Option<StoredSidebarView>,
    /// Height of the Source Control graph.
    history_graph_height: Option<f32>,
    /// Whether the Source Control graph was visible.
    history_graph_open: Option<bool>,
    /// Whether the Source Control changes section was expanded.
    changes_section_open: Option<bool>,
    /// Whether the Graph includes every history reference.
    history_all: Option<bool>,
    /// Logical width of the window when it is not maximized.
    window_width: Option<f32>,
    /// Logical height of the window when it is not maximized.
    window_height: Option<f32>,
    /// Whether the window filled the screen it was on.
    window_maximized: Option<bool>,
}

/// One language server as it is written down.
///
/// A server that takes no arguments is written as the command alone, which
/// is what nearly all of them are; one that takes arguments spells them out.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum StoredServer {
    /// The command, run with no arguments.
    Command(String),
    /// The command, the arguments to run it with, and what to configure it
    /// with as it starts.
    Invocation {
        /// The program to run.
        command: String,
        /// The arguments to run it with.
        arguments: Vec<String>,
        /// What the server is configured with as it starts.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<serde_norway::Value>,
    },
}

impl StoredServer {
    /// The server this stands for, named for as long as the editor runs.
    fn into_server(self) -> Server {
        let (command, arguments, options) = match self {
            Self::Command(command) => (command, Vec::new(), None),
            Self::Invocation {
                command,
                arguments,
                options,
            } => (command, arguments, options),
        };
        Server {
            command: command.leak(),
            arguments: arguments
                .into_iter()
                .map(|argument| &*argument.leak())
                .collect::<Vec<_>>()
                .leak(),
            options: options
                .and_then(|options| serde_json::to_string(&options).ok())
                .map_or(pm_text::NO_OPTIONS, |options| &*options.leak()),
        }
    }

    /// How `server` is written down.
    fn of(server: &Server) -> Self {
        let options = serde_json::from_str::<serde_norway::Value>(server.options).ok();
        let options = options.filter(|options| !matches!(options, serde_norway::Value::Null));
        if server.arguments.is_empty() && options.is_none() {
            return Self::Command(server.command.to_owned());
        }
        Self::Invocation {
            command: server.command.to_owned(),
            arguments: server
                .arguments
                .iter()
                .map(|&argument| argument.to_owned())
                .collect(),
            options,
        }
    }
}

impl Stored {
    /// What this file stands for, defaulting anything it leaves out.
    pub(super) fn into_restored(self) -> Restored {
        Restored {
            projects: self.projects.clone().unwrap_or_default(),
            active: self.active_project.clone(),
            layout: self.layout(),
            window: self.window(),
            panes: self.panes.clone().unwrap_or_default(),
            language_servers: self.language_servers(),
            bootstrap: self.bootstrap(),
            onboarded: self.finished.unwrap_or_default(),
            preferences: self.into_preferences(),
        }
    }

    /// The servers this file puts in place of the ones languages name.
    fn language_servers(&self) -> BTreeMap<String, Vec<Server>> {
        self.language_servers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(language, servers)| {
                let servers = servers.into_iter().map(StoredServer::into_server).collect();
                (language, servers)
            })
            .collect()
    }

    /// What this file gives a fresh worktree, defaulting what it leaves out.
    fn bootstrap(&self) -> Bootstrap {
        let defaults = Bootstrap::default();
        Bootstrap {
            link: self.worktree_link.clone().unwrap_or(defaults.link),
            copy: self.worktree_copy.clone().unwrap_or(defaults.copy),
            port: self.worktree_port.clone().or(defaults.port),
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
            secondary_sidebar_view: self.secondary_sidebar_view.map_or(
                defaults.secondary_sidebar_view,
                StoredSidebarView::into_view,
            ),
            history_graph_height: self
                .history_graph_height
                .unwrap_or(defaults.history_graph_height),
            history_graph_open: self
                .history_graph_open
                .unwrap_or(defaults.history_graph_open),
            changes_section_open: self
                .changes_section_open
                .unwrap_or(defaults.changes_section_open),
            history_all: self.history_all.unwrap_or(defaults.history_all),
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
    fn into_preferences(self) -> Preferences {
        let defaults = Preferences::default();
        Preferences {
            theme_mode: self.theme_mode.unwrap_or(defaults.theme_mode),
            theme_family: self
                .theme_family
                .as_deref()
                .and_then(family_index)
                .unwrap_or(defaults.theme_family),
            keymap: self.keymap.unwrap_or(defaults.keymap),
            vim_mode: self.vim_mode.unwrap_or(defaults.vim_mode),
            format_on_save: self.format_on_save.unwrap_or(defaults.format_on_save),
            trust_worktrees: self.trust_worktrees.unwrap_or(defaults.trust_worktrees),
            metrics: self.metrics.unwrap_or(defaults.metrics),
            crash_reports: self.crash_reports.unwrap_or(defaults.crash_reports),
        }
    }
}

impl Stored {
    /// The file to write for the window as it stands.
    pub(super) fn of(restored: &Restored) -> Self {
        let Restored {
            preferences,
            onboarded,
            projects,
            active,
            layout,
            panes,
            window,
            language_servers,
            bootstrap,
        } = restored;

        Self {
            theme_mode: Some(preferences.theme_mode),
            theme_family: Some(pm_ui::family(preferences.theme_family).name.to_owned()),
            keymap: Some(preferences.keymap),
            vim_mode: Some(preferences.vim_mode),
            format_on_save: Some(preferences.format_on_save),
            trust_worktrees: Some(preferences.trust_worktrees),
            metrics: Some(preferences.metrics),
            crash_reports: Some(preferences.crash_reports),
            language_servers: (!language_servers.is_empty()).then(|| {
                language_servers
                    .iter()
                    .map(|(language, servers)| {
                        let servers = servers.iter().map(StoredServer::of).collect();
                        (language.clone(), servers)
                    })
                    .collect()
            }),
            worktree_link: Some(bootstrap.link.clone()),
            worktree_copy: Some(bootstrap.copy.clone()),
            worktree_port: bootstrap.port.clone(),
            finished: Some(*onboarded),
            projects: Some(projects.clone()),
            active_project: active.clone(),
            panes: Some(panes.clone()),
            primary_sidebar_open: Some(layout.primary_sidebar_open),
            primary_sidebar_width: Some(layout.primary_sidebar_width),
            bottom_panel_open: Some(layout.bottom_panel_open),
            bottom_panel_height: Some(layout.bottom_panel_height),
            secondary_sidebar_open: Some(layout.secondary_sidebar_open),
            secondary_sidebar_width: Some(layout.secondary_sidebar_width),
            secondary_sidebar_view: Some(StoredSidebarView::of(layout.secondary_sidebar_view)),
            history_graph_height: Some(layout.history_graph_height),
            history_graph_open: Some(layout.history_graph_open),
            changes_section_open: Some(layout.changes_section_open),
            history_all: Some(layout.history_all),
            window_width: Some(window.width),
            window_height: Some(window.height),
            window_maximized: Some(window.maximized),
        }
    }
}

/// Which list the sidebar beside the panes was showing, as it is written down.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum StoredSidebarView {
    /// Every file of the worktree.
    Files,
    /// Everything that has changed in it.
    Changes,
}

impl StoredSidebarView {
    /// The written name of the view the sidebar was showing.
    fn of(view: SidebarView) -> Self {
        match view {
            SidebarView::Files => Self::Files,
            SidebarView::Changes => Self::Changes,
        }
    }

    /// The view the written name stands for.
    fn into_view(self) -> SidebarView {
        match self {
            Self::Files => SidebarView::Files,
            Self::Changes => SidebarView::Changes,
        }
    }
}

/// The index of the family called `name`, of the ones on offer.
fn family_index(name: &str) -> Option<usize> {
    families().iter().position(|family| family.name == name)
}
