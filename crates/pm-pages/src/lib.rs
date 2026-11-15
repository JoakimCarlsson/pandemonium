//! Pandemonium's pages, built only from `pm-components`: the top bar, the
//! sidebar, the session view, the welcome page for new work, the history,
//! extensions and project overview pages, the right panel with its Git,
//! Files and Search tabs, the session's diff and open files, the dock, the
//! quick input, the dialogs and the window that holds them; and the
//! settings window and the folder picker, each in its own window.

mod actions;
mod adder;
mod browser_pane;
mod browser_safety;
mod changes_page;
mod conflict_page;
mod conversation;
mod debug_panel;
mod dialogs;
mod diff_page;
mod dock;
mod extensions;
mod file_view;
mod files_panel;
mod folder_picker;
mod git_panel;
mod history;
mod overview;
mod problems_panel;
mod prompt_box;
mod quick_input;
mod search_panel;
mod session;
mod session_asks;
mod session_extras;
mod session_tabs;
mod settings;
mod settings_actions;
mod settings_agents;
mod settings_body;
mod settings_forms;
mod settings_library;
mod settings_nav;
mod settings_rows;
mod side_panel;
mod sidebar;
mod status_bar;
mod tasks_panel;
mod top_bar;
mod welcome;
mod window_theme;
mod workspace;

pub use actions::{Actions, Destination, Panel, SignInAction, UpdateAction, WindowAction};
pub use browser_pane::browser_pane;
pub use dialogs::dialogs;
pub use diff_page::diff_page;
pub use dock::dock;
pub use extensions::extensions_page;
pub use file_view::open_file_view;
pub use files_panel::files_panel;
pub use folder_picker::{PICKER_SIZE, PickerActions, folder_picker_page};
pub use git_panel::git_panel;
pub use history::history_page;
pub use overview::overview_page;
pub use quick_input::quick_input;
pub use search_panel::search_panel;
pub use session::session_view;
pub use session_tabs::{session_tabs, tab_key};
pub use settings::{SETTINGS_SIZE, settings_page, settings_section};
pub use settings_actions::SettingsActions;
pub use side_panel::side_panel;
pub use sidebar::sidebar;
pub use top_bar::{TOP_BAR_HEIGHT, top_bar};
pub use welcome::welcome_page;
pub use window_theme::window_theme;
pub use workspace::workspace_page;
