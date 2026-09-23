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

use crate::config::fonts::Fonts;
use crate::config::theme::StoredOverrides;
use crate::config::{Preferences, Restored, ThemeMode, VimBinding, WindowState};
use crate::editor::{CursorShape, Display};
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
    /// The colours repainted over whichever family is chosen.
    #[serde(skip_serializing_if = "Option::is_none")]
    theme_overrides: Option<StoredOverrides>,
    /// The family prose and labels are set in.
    #[serde(skip_serializing_if = "Option::is_none")]
    ui_font_family: Option<String>,
    /// The body size of the interface.
    ui_font_size: Option<f32>,
    /// The family code is set in.
    #[serde(skip_serializing_if = "Option::is_none")]
    buffer_font_family: Option<String>,
    /// The size code is set in.
    buffer_font_size: Option<f32>,
    /// The weight code is set in.
    buffer_font_weight: Option<u16>,
    /// The distance between two lines of code, as a multiple of its size.
    buffer_line_height: Option<f32>,
    /// The size a terminal is set in.
    terminal_font_size: Option<f32>,
    /// How many lines of scrollback a terminal keeps.
    terminal_scrollback: Option<usize>,
    /// The keymap the editor starts from.
    keymap: Option<BaseKeymap>,
    /// Whether editing starts in vim mode.
    vim_mode: Option<bool>,
    /// How much vim's unnamed register shares with the system clipboard.
    vim_clipboard: Option<StoredClipboardUse>,
    /// The reader's own vim bindings.
    #[serde(skip_serializing_if = "Option::is_none")]
    vim_keymap: Option<Vec<StoredVimBinding>>,
    /// How wide a step of indentation is where a file does not say.
    tab_size: Option<usize>,
    /// Whether a step of indentation is a tab where a file does not say.
    hard_tabs: Option<bool>,
    /// Whether the gutter numbers the lines.
    line_numbers: Option<bool>,
    /// Whether the numbers count away from the cursor's line.
    relative_line_numbers: Option<bool>,
    /// Whether the cursor's line is washed.
    current_line_highlight: Option<bool>,
    /// Whether the other places the word at the cursor appears are washed.
    occurrence_highlight: Option<bool>,
    /// Whether a line is drawn at every step of indentation.
    indent_guides: Option<bool>,
    /// Whether the lines the view is inside stay pinned above it.
    sticky_scroll: Option<bool>,
    /// Whether the scrollbars are drawn.
    scrollbars: Option<bool>,
    /// The column a guide is drawn down, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    wrap_guide: Option<usize>,
    /// Whether a language server's hints are written into the lines.
    inlay_hints: Option<bool>,
    /// How the caret is drawn.
    cursor_shape: Option<StoredCursorShape>,
    /// Whether the caret blinks.
    cursor_blink: Option<bool>,
    /// How far a notch of the wheel scrolls, against its usual distance.
    scroll_sensitivity: Option<f32>,
    /// Whether a file is laid out the way its formatter would when it is saved.
    format_on_save: Option<bool>,
    /// Whether the space at the ends of lines goes when a file is saved.
    remove_trailing_whitespace_on_save: Option<bool>,
    /// Whether a saved file always ends in a line break.
    ensure_final_newline_on_save: Option<bool>,
    /// Whether a new session's worktree is trusted without being asked about.
    trust_worktrees: Option<bool>,
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
        let bootstrap = self.bootstrap();
        Preferences {
            theme_mode: self.theme_mode.unwrap_or(defaults.theme_mode),
            theme_family: self
                .theme_family
                .as_deref()
                .and_then(family_index)
                .unwrap_or(defaults.theme_family),
            theme_overrides: self
                .theme_overrides
                .map_or(defaults.theme_overrides, StoredOverrides::into_overrides),
            fonts: Fonts {
                interface_family: self.ui_font_family.or(defaults.fonts.interface_family),
                interface_size: self.ui_font_size.unwrap_or(defaults.fonts.interface_size),
                buffer_family: self.buffer_font_family.or(defaults.fonts.buffer_family),
                buffer_size: self.buffer_font_size.unwrap_or(defaults.fonts.buffer_size),
                buffer_weight: self
                    .buffer_font_weight
                    .unwrap_or(defaults.fonts.buffer_weight),
                buffer_line_height: self
                    .buffer_line_height
                    .unwrap_or(defaults.fonts.buffer_line_height),
                terminal_size: self
                    .terminal_font_size
                    .unwrap_or(defaults.fonts.terminal_size),
            },
            keymap: self.keymap.unwrap_or(defaults.keymap),
            vim_mode: self.vim_mode.unwrap_or(defaults.vim_mode),
            vim_clipboard: self
                .vim_clipboard
                .map_or(defaults.vim_clipboard, StoredClipboardUse::into_use),
            vim_bindings: self.vim_keymap.as_ref().map_or_else(Vec::new, |bindings| {
                bindings.iter().map(StoredVimBinding::to_binding).collect()
            }),
            tab_size: self.tab_size.unwrap_or(defaults.tab_size),
            hard_tabs: self.hard_tabs.unwrap_or(defaults.hard_tabs),
            display: Display {
                line_numbers: self.line_numbers.unwrap_or(defaults.display.line_numbers),
                relative_line_numbers: self
                    .relative_line_numbers
                    .unwrap_or(defaults.display.relative_line_numbers),
                current_line: self
                    .current_line_highlight
                    .unwrap_or(defaults.display.current_line),
                occurrences: self
                    .occurrence_highlight
                    .unwrap_or(defaults.display.occurrences),
                indent_guides: self.indent_guides.unwrap_or(defaults.display.indent_guides),
                sticky_scroll: self.sticky_scroll.unwrap_or(defaults.display.sticky_scroll),
                scrollbars: self.scrollbars.unwrap_or(defaults.display.scrollbars),
                wrap_guide: self.wrap_guide.or(defaults.display.wrap_guide),
                cursor_shape: self
                    .cursor_shape
                    .map_or(defaults.display.cursor_shape, StoredCursorShape::into_shape),
                whole_lines: false,
            },
            inlay_hints: self.inlay_hints.unwrap_or(defaults.inlay_hints),
            cursor_blink: self.cursor_blink.unwrap_or(defaults.cursor_blink),
            scroll_sensitivity: self
                .scroll_sensitivity
                .unwrap_or(defaults.scroll_sensitivity),
            format_on_save: self.format_on_save.unwrap_or(defaults.format_on_save),
            trim_whitespace: self
                .remove_trailing_whitespace_on_save
                .unwrap_or(defaults.trim_whitespace),
            final_newline: self
                .ensure_final_newline_on_save
                .unwrap_or(defaults.final_newline),
            terminal_scrollback: self
                .terminal_scrollback
                .unwrap_or(defaults.terminal_scrollback),
            trust_worktrees: self.trust_worktrees.unwrap_or(defaults.trust_worktrees),
            bootstrap,
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
        } = restored;
        let bootstrap = &preferences.bootstrap;
        let (fonts, display) = (&preferences.fonts, &preferences.display);
        let overrides = StoredOverrides::of(&preferences.theme_overrides);

        Self {
            theme_mode: Some(preferences.theme_mode),
            theme_family: Some(pm_ui::family(preferences.theme_family).name.to_owned()),
            theme_overrides: (!overrides.is_empty()).then_some(overrides),
            ui_font_family: fonts.interface_family.clone(),
            ui_font_size: Some(fonts.interface_size),
            buffer_font_family: fonts.buffer_family.clone(),
            buffer_font_size: Some(fonts.buffer_size),
            buffer_font_weight: Some(fonts.buffer_weight),
            buffer_line_height: Some(fonts.buffer_line_height),
            terminal_font_size: Some(fonts.terminal_size),
            terminal_scrollback: Some(preferences.terminal_scrollback),
            keymap: Some(preferences.keymap),
            vim_mode: Some(preferences.vim_mode),
            vim_clipboard: Some(StoredClipboardUse::of(preferences.vim_clipboard)),
            vim_keymap: (!preferences.vim_bindings.is_empty()).then(|| {
                preferences
                    .vim_bindings
                    .iter()
                    .map(StoredVimBinding::of)
                    .collect()
            }),
            tab_size: Some(preferences.tab_size),
            hard_tabs: Some(preferences.hard_tabs),
            line_numbers: Some(display.line_numbers),
            relative_line_numbers: Some(display.relative_line_numbers),
            current_line_highlight: Some(display.current_line),
            occurrence_highlight: Some(display.occurrences),
            indent_guides: Some(display.indent_guides),
            sticky_scroll: Some(display.sticky_scroll),
            scrollbars: Some(display.scrollbars),
            wrap_guide: display.wrap_guide,
            inlay_hints: Some(preferences.inlay_hints),
            cursor_shape: Some(StoredCursorShape::of(display.cursor_shape)),
            cursor_blink: Some(preferences.cursor_blink),
            scroll_sensitivity: Some(preferences.scroll_sensitivity),
            format_on_save: Some(preferences.format_on_save),
            remove_trailing_whitespace_on_save: Some(preferences.trim_whitespace),
            ensure_final_newline_on_save: Some(preferences.final_newline),
            trust_worktrees: Some(preferences.trust_worktrees),
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

/// How much vim's unnamed register shares with the system clipboard, as it
/// is written down.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum StoredClipboardUse {
    /// Every yank and delete.
    Always,
    /// Only yanks.
    OnYank,
    /// Nothing but `"+` and `"*`.
    Never,
}

impl StoredClipboardUse {
    /// The written name of `sharing`.
    fn of(sharing: pm_vim::ClipboardUse) -> Self {
        match sharing {
            pm_vim::ClipboardUse::Always => Self::Always,
            pm_vim::ClipboardUse::OnYank => Self::OnYank,
            pm_vim::ClipboardUse::Never => Self::Never,
        }
    }

    /// The choice the written name stands for.
    fn into_use(self) -> pm_vim::ClipboardUse {
        match self {
            Self::Always => pm_vim::ClipboardUse::Always,
            Self::OnYank => pm_vim::ClipboardUse::OnYank,
            Self::Never => pm_vim::ClipboardUse::Never,
        }
    }
}

/// One of the reader's vim bindings, as it is written down.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredVimBinding {
    /// The keystrokes.
    keys: String,
    /// The action.
    action: String,
    /// When it applies, normal mode when left out.
    #[serde(default = "normal_mode")]
    when: String,
}

impl StoredVimBinding {
    /// The written form of `binding`.
    fn of(binding: &VimBinding) -> Self {
        Self {
            keys: binding.keys.clone(),
            action: binding.action.clone(),
            when: binding.when.clone(),
        }
    }

    /// The binding the written form stands for.
    fn to_binding(&self) -> VimBinding {
        VimBinding {
            keys: self.keys.clone(),
            action: self.action.clone(),
            when: self.when.clone(),
        }
    }
}

/// The clause a binding written without one applies in.
fn normal_mode() -> String {
    "normal".to_owned()
}

/// How the caret is drawn, as it is written down.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum StoredCursorShape {
    /// A thin upright line between two characters.
    Bar,
    /// A cell-wide block over the character after the caret.
    Block,
    /// A line under the character after the caret.
    Underline,
}

impl StoredCursorShape {
    /// The written name of `shape`.
    fn of(shape: CursorShape) -> Self {
        match shape {
            CursorShape::Bar => Self::Bar,
            CursorShape::Block => Self::Block,
            CursorShape::Underline => Self::Underline,
        }
    }

    /// The shape the written name stands for.
    fn into_shape(self) -> CursorShape {
        match self {
            Self::Bar => CursorShape::Bar,
            Self::Block => CursorShape::Block,
            Self::Underline => CursorShape::Underline,
        }
    }
}

/// The index of the family called `name`, of the ones on offer.
pub(super) fn family_index(name: &str) -> Option<usize> {
    families().iter().position(|family| family.name == name)
}
