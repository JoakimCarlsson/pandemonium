//! What the window does with its preferences modal: opening it, scrolling it,
//! asking for what a row cannot take by itself, and handing what changed to
//! the parts of the window that act on it.

use pm_ui::{Element, Styled, Theme};

use crate::app::{App, Writing};
use crate::config::{self, FontSlot, WorktreePaths};
use crate::desktop;
use crate::editor::Habits;
use crate::message::Message;
use crate::panes::{SavedKind, SavedNode};
use crate::picker::{Choice, Kind, Row};
use crate::settings::{AgentList, McpPage, SettingsPane, Subject, settings_pane};
use crate::theme::{self, TOKENS};
use crate::workspace::MenuTarget;

impl App {
    /// Opens the preferences modal above the current workspace.
    pub(super) fn open_settings(&mut self) {
        if !self.onboarded {
            return;
        }
        self.dismiss_picker();
        self.dismiss_popup();
        self.menu = None;
        self.search_focused = false;
        self.tree_edit = None;
        self.release_pane_focus();
        self.resolver.reset();
        self.settings_open = true;
        if let Some(ui) = self.ui.as_mut() {
            ui.clear_focus();
            ui.clear_text_selection();
        }
        if !self.languages.requested {
            self.refresh_language_catalogue();
        }
    }

    /// Closes preferences and returns keyboard focus to the current workspace pane.
    pub(super) fn close_settings(&mut self) {
        self.settings_open = false;
        self.resolver.reset();
        self.settings.stop_recording();
        self.dismiss_picker();
        self.menu = None;
        self.writing = None;
        if let Some(ui) = self.ui.as_mut() {
            ui.clear_focus();
            ui.clear_text_selection();
        }
        self.focus_pane(self.panes.focus());
    }

    /// The centred modal bounds, fitted to the available window size.
    pub(super) fn settings_bounds(&self, window: pm_gfx::Size) -> pm_gfx::Rect {
        let width = (window.width - 48.0).clamp(1.0, 1200.0);
        let height = (window.height - 64.0).clamp(1.0, 900.0);
        pm_gfx::Rect::from_xywh(
            (window.width - width) / 2.0,
            (window.height - height) / 2.0,
            width,
            height,
        )
    }

    /// Draws a dimmed workspace and a large preferences card above it.
    pub(super) fn settings_overlay(
        &self,
        theme: &Theme,
        window: pm_gfx::Size,
    ) -> crate::workspace::Overlaid {
        let bounds = self.settings_bounds(window);
        crate::workspace::Overlaid {
            at: pm_gfx::Point::new(0.0, 0.0),
            content: Box::new(
                pm_ui::v_flex()
                    .block_pointer()
                    .w_px((window.width - 8.0).max(1.0))
                    .h_px((window.height - 8.0).max(1.0))
                    .on_click(Message::CloseSettings)
                    .on_secondary_click(Message::CloseSettings)
                    .bg(pm_gfx::Rgba::new(0.0, 0.0, 0.0, 0.55))
                    .items_center()
                    .justify_center()
                    .child(crate::settings::settings_modal(
                        theme,
                        self.settings_content(theme),
                        bounds.size,
                    )),
            ),
            backdrop: Some(Message::CloseSettings),
            above: false,
        }
    }

    /// Builds the preferences pages inside the modal.
    pub(super) fn settings_content(&self, theme: &Theme) -> Box<dyn Element<Message>> {
        let catalog = self.mcp_catalog();
        let usage = self.mcp_usage();
        let agent_catalog = self.agent_catalog();
        let focus = match self.writing {
            Some(Writing::FormField(field)) => Some(field),
            _ => None,
        };
        settings_pane(
            theme,
            &SettingsPane {
                languages: crate::settings::languages::LanguagesPage {
                    state: &self.languages,
                    field: match self.writing {
                        Some(Writing::LanguageServerField(index)) => Some(index),
                        _ => None,
                    },
                    solid: self.caret_solid(),
                    preferences: &self.preferences,
                    servers: &self.language_servers,
                    selected: self.settings_language(),
                },
                settings: &self.settings,
                preferences: &self.preferences,
                keymap: self.resolver.keymap(),
                file: crate::config::settings_file(),
                agents: AgentList {
                    agents: pm_acp::agents(),
                    custom: &self.agent_servers,
                    catalog: &agent_catalog,
                    search: &self.agent_search,
                    typing: self.writing == Some(Writing::AgentSearch),
                    installed_open: self.settings.agents_installed_open(),
                    available_open: self.settings.agents_available_open(),
                    form: self
                        .server_form
                        .as_ref()
                        .filter(|form| form.subject == Subject::Agent),
                    focus,
                    solid: self.caret_solid(),
                },
                mcp: McpPage {
                    form: self
                        .server_form
                        .as_ref()
                        .filter(|form| form.subject == Subject::McpServer),
                    focus,
                    servers: &self.mcp_servers,
                    usage: &usage,
                    catalog: &catalog,
                    search: &self.mcp_search,
                    typing: self.writing == Some(Writing::McpSearch),
                    solid: self.caret_solid(),
                    installed_open: self.settings.installed_open(),
                    available_open: self.settings.available_open(),
                },
            },
        )
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
        if !self.settings_open {
            return false;
        }
        let window = self
            .renderer
            .as_ref()
            .map_or(pm_gfx::Size::zero(), pm_gfx::Renderer::size);
        if self
            .pointer
            .is_none_or(|at| self.settings_bounds(window).contains(at))
        {
            self.settings.scroll_by(delta);
        }
        true
    }

    /// Carries out a message of the settings pane that asks rather than
    /// sets, saying whether `message` was one.
    pub(super) fn settings_command(&mut self, message: Message) -> bool {
        match message {
            Message::ScrollSettings(event, step) => self.settings.drag_scroll(event, step),
            Message::ToggleBuiltinLanguages => {
                self.languages.builtin_open = !self.languages.builtin_open;
                self.writing = None;
            }
            Message::SetLanguageFilter(filter) => {
                self.languages.filter = filter;
                self.writing = None;
            }
            Message::OpenLanguageSource(index, installed) => {
                let entries = if installed {
                    config::extensions::installed()
                } else {
                    self.languages.catalogue.clone()
                };
                if let Some(entry) = entries.get(index) {
                    desktop::browse(&entry.source);
                }
            }
            Message::RefreshLanguageCatalogue => self.refresh_language_catalogue(),
            Message::InstallLanguageExtension(index) => self.install_language_extension(index),
            Message::ReinstallLanguageExtension(index) => self.reinstall_language_extension(index),
            Message::ImportLanguageExtension => self.import_language_extension(),
            Message::RemoveLanguageExtension(index) => self.remove_language_extension(index),
            Message::PickSettingsLanguage => self.pick_settings_language(),
            Message::ToggleLanguageSetting(setting) => self.toggle_language_setting(setting),
            Message::StepLanguageSetting(setting, step) => {
                self.step_language_setting(setting, step);
            }
            Message::ResetLanguageSetting(setting) => self.reset_language_setting(setting),
            Message::ResetLanguageSettings => self.reset_language_settings(),
            Message::SetLanguageFormatter(kind) => self.set_language_formatter(kind),
            Message::AskLanguageFormatter => self.ask_language_formatter(),
            Message::AddLanguageServer => self.edit_language_server(None),
            Message::EditLanguageServer(at) => self.edit_language_server(Some(at)),
            Message::RemoveLanguageServer(at) => self.remove_language_server(at),
            Message::ResetLanguageServers => self.reset_language_servers(),
            Message::SaveLanguageServer => self.save_language_server(),
            Message::CancelLanguageServer => {
                self.languages.editor = None;
                self.writing = None;
            }
            Message::FocusLanguageServerField(index) => {
                self.write_in(Writing::LanguageServerField(index));
            }
            Message::WriteLanguageServerField(index, phase, anchor, head) => {
                self.point_in(Writing::LanguageServerField(index), phase, anchor, head);
            }
            Message::CopyVersion => desktop::copy(crate::build_info::description()),
            Message::PickFont(slot) => self.ask_font(slot),
            Message::EditThemeColor(token) => self.ask_theme_color(token),
            Message::SaveTheme => self.ask_theme_name(),
            Message::ReloadThemes => {
                config::reload_themes(&mut self.preferences);
                self.store();
            }
            Message::SaveKeymap => self.ask_keymap_name(),
            Message::ReloadKeymaps => {
                config::reload_keymaps(&mut self.preferences);
                self.follow_keymap();
            }
            Message::ReloadExtensions => {
                for error in config::reload_extensions(&mut self.preferences) {
                    self.notices.trouble(error, None);
                }
                self.follow_keymap();
                self.activate_languages();
            }
            Message::WriteAgentSearch(phase, anchor, head) => {
                self.point_in(Writing::AgentSearch, phase, anchor, head);
            }
            Message::InstallAgent(index) => self.install_available_agent(index),
            Message::ToggleAgentsInstalled => self.settings.toggle_agents_installed(),
            Message::ToggleAgentsAvailable => self.settings.toggle_agents_available(),
            Message::AddAgentServer => self.add_agent_server(),
            Message::EditAgentServer(index) => self.edit_agent_server(index),
            Message::RemoveAgentServer(index) => self.remove_agent_server(index),
            Message::ShowAgentServerMenu(index) => self.open_menu(MenuTarget::AgentServer(index)),
            Message::AddMcpServer => self.add_mcp_server(),
            Message::InstallMcpServer(index) => self.install_mcp_server(index),
            Message::ToggleMcpServer(index) => self.toggle_mcp_server(index),
            Message::WriteFormField(field, phase, anchor, head) => {
                self.point_in(Writing::FormField(field), phase, anchor, head);
            }
            Message::AddFormVariable => self.add_form_variable(),
            Message::SuggestFormVariable(place) => self.suggest_form_variable(place),
            Message::RemoveFormVariable(at) => self.remove_form_variable(at),
            Message::SaveServerForm => self.save_server_form(),
            Message::CancelServerForm => self.cancel_server_form(),
            Message::CopyMcpConfiguration(index) => self.copy_mcp_configuration(index),
            Message::OpenMcpWebsite(index) => self.open_mcp_website(index),
            Message::RevealSettingsFile => self.reveal_settings_file(),
            Message::ShowMcpServerMenu(index) => self.open_menu(MenuTarget::McpServer(index)),
            Message::ToggleMcpInstalled => self.settings.toggle_installed(),
            Message::ToggleMcpAvailable => self.settings.toggle_available(),
            Message::OpenMcpDocs => desktop::browse("https://modelcontextprotocol.io"),
            Message::WriteMcpSearch(phase, anchor, head) => {
                self.point_in(Writing::McpSearch, phase, anchor, head);
            }
            Message::EditMcpServer(index) => self.edit_mcp_server(index),
            Message::RemoveMcpServer(index) => self.remove_mcp_server(index),
            Message::RecordBinding(action) => self.settings.record(action),
            Message::UnbindAction(action) => {
                self.preferences.unbind(action);
                self.follow_keymap();
            }
            _ => return false,
        }
        true
    }

    /// Hands the preferences to everything that keeps a copy of one: the
    /// open files, the shells, and the hints already written into lines.
    pub(super) fn follow_preferences(&mut self) {
        let preferences = &self.preferences;
        let habits = |name: Option<&str>| {
            let settings = preferences.language(name);
            Habits {
                indent: settings.indent,
                indent_fixed: settings.indent_fixed,
                trim_whitespace: settings.trim_whitespace,
                final_newline: settings.final_newline,
            }
        };
        let by_language = preferences
            .language_overrides()
            .keys()
            .map(|name| (name.clone(), habits(Some(name))))
            .collect();
        self.editor.set_habits(habits(None), by_language);
        self.terminals
            .set_scrollback(preferences.terminal_scrollback);
        if !preferences.inlay_hints {
            self.editor.forget_hints();
        }
        if !preferences.code_lens {
            self.editor.forget_lenses();
        }
        if !preferences.edit_predictions.enabled {
            self.editor.dismiss_predictions();
        }
        self.vim.share_clipboard(self.preferences.vim_clipboard);
        self.bind_vim_keys();
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
        self.open_picker_with(Kind::ThemeColor(token), Vec::new(), theme::hex(color));
    }

    /// Repaints `token` in the colour typed, over the appearance in front.
    ///
    /// What does not read as a colour leaves the colour as it was. The
    /// colour the family itself gives it is no override at all, so typing
    /// that takes the override away rather than writing it down.
    pub(super) fn set_theme_color(&mut self, token: usize, typed: &str) {
        let (Some(color), Some(found)) = (theme::from_hex(typed), TOKENS.get(token)) else {
            return;
        };
        let appearance = self.theme().appearance;
        let family = theme::family(self.preferences.theme_family).variant(appearance);
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
            theme::family(self.preferences.theme_family).name
        );
        self.open_picker_with(Kind::ThemeName, Vec::new(), named);
    }

    /// Asks what to call the keymap being pressed, before writing it down.
    fn ask_keymap_name(&mut self) {
        let named = format!("{} Custom", crate::keymap::name(self.preferences.keymap));
        self.open_picker_with(Kind::KeymapName, Vec::new(), named);
    }

    /// Writes the keymap being pressed down as one called `typed`.
    pub(super) fn save_keymap(&mut self, typed: &str) {
        if config::save_keymap(&mut self.preferences, typed) {
            self.follow_keymap();
        }
    }

    /// Puts the keymap the preferences name in force, and writes them down.
    pub(super) fn follow_keymap(&mut self) {
        self.resolver.set_keymap(self.preferences.keymap_in_force());
        self.store();
    }

    /// Takes a keypress while a binding is being recorded, saying whether
    /// one is.
    ///
    /// Every chord is the binding's, bar three unmodified keys: Enter keeps
    /// what was pressed, Escape lets it go, and Backspace takes the last
    /// chord back. Those three can still be bound by writing a keymap. A
    /// recording the reader has left the settings pane behind is let go.
    pub(super) fn record_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if self.settings.recording().is_none() {
            return false;
        }
        if !self.settings_open {
            self.settings.stop_recording();
            return false;
        }
        let Some(chord) = crate::keymap::chord(event, self.modifiers) else {
            return true;
        };
        let plain = |named| chord == crate::keymap::Chord::plain(crate::keymap::Key::Named(named));
        if plain(crate::keymap::Named::Escape) {
            self.settings.stop_recording();
        } else if plain(crate::keymap::Named::Backspace) {
            self.settings.erase();
        } else if plain(crate::keymap::Named::Enter) {
            let recorded = self.settings.stop_recording();
            if let Some((action, sequence)) =
                recorded.and_then(|recorded| Some((recorded.action, recorded.sequence()?)))
            {
                self.preferences.rebind(action, sequence);
                self.follow_keymap();
            }
        } else {
            self.settings.press(chord);
        }
        true
    }

    /// Writes the theme being drawn in down as one called `typed`.
    pub(super) fn save_theme(&mut self, typed: &str) {
        if config::save_theme(&mut self.preferences, typed) {
            self.store();
        }
    }
}

/// Whether an older saved layout had preferences in front of any pane.
pub(super) fn settings_was_open(node: &SavedNode) -> bool {
    match node {
        SavedNode::Pane { tabs } => tabs
            .iter()
            .any(|tab| tab.kind == SavedKind::Settings && tab.front),
        SavedNode::Split { children, .. } => children.iter().any(settings_was_open),
    }
}
