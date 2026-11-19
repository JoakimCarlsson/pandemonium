//! The settings window: the sections of settings down its left side and the one chosen on its right, filling a window of its own.

use crate::areas::SettingsEvent;
use crate::kit::{self, Frame};
use crate::theme::Indicated;

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{CursorIcon, Modifier};
use rustle::{Icon, phosphor};

use crate::events::Interactive;
use crate::theme::{Radius, Role};

use crate::input::Input;
use crate::parts::{clipped, glyph, line};
use crate::settings_rows::SECTIONS;
use crate::settings_servers;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::tile::Tile;

/// How far the settings stand in from either side of the right of the window, in logical pixels.
const PAGE_INSET: f32 = 32.0;

/// How wide the list of sections is, in logical pixels.
const NAVIGATION_WIDTH: f32 = 220.0;

/// How wide the list of sections is when the window is narrow, in logical pixels.
const NARROW_NAVIGATION: f32 = 184.0;

/// How wide the window is at most for the list of sections to be narrow, in logical pixels.
const NARROW_WINDOW: f32 = 760.0;

/// A group of settings, named down the left side of the window; some sit inside another.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Section {
    /// How the window opens, closes and keeps itself up to date.
    #[default]
    General,
    /// How the window looks: its theme, its fonts and how closely it sets things.
    Appearance,
    /// How files are written in, with the groups below it.
    Editor,
    /// How files look.
    EditorAppearance,
    /// How files are edited.
    EditorEditing,
    /// What saving does.
    EditorSaving,
    /// What the language server adds.
    EditorIntelligence,
    /// Finding and scrolling.
    EditorSearch,
    /// Editing with vim's keys.
    EditorVim,
    /// How each language is edited, and the languages installed and offered.
    Languages,
    /// The keys that work the editor.
    Keymap,
    /// The shell the terminal starts and how it is drawn.
    Terminal,
    /// The embedded browser: turning it on, opening dev servers in it and what agents may do with it.
    Browser,
    /// How git is run and what it marks.
    Git,
    /// How sessions get their branches and worktrees.
    Worktrees,
    /// What agents are started with, with the groups below it.
    Agents,
    /// The agents that can be started, and the ones added by hand.
    AgentServers,
    /// The tool servers every agent is started with.
    McpServers,
    /// The OpenAI-compatible model servers the reader declared, each offered as an agent.
    ModelServers,
    /// The model slots: which model each kind of work asks, with backups, and the presets that set them all.
    ModelSlots,
    /// Orchestrator mode: whether the agent leads, and the work categories it hands work to, each with its model.
    Orchestration,
    /// What the sessions used: tokens, requests and spend for each day, each model and each session.
    Usage,
    /// How many of each kind of customization agents are given in the project.
    Customizations,
    /// The skills agents are given.
    Skills,
    /// The instruction files agents read.
    Instructions,
    /// The agent definitions agents can hand work to.
    AgentDefinitions,
    /// The commands agents run when something happens.
    Hooks,
    /// The tools agents are given, in sets by where they come from.
    Tools,
    /// The servers the window knows and where new projects go.
    Servers,
    /// The settings a search found, on whatever page they are.
    Results,
}

impl Section {
    /// Every section, in the order the list shows them, each followed by the ones inside it.
    pub const ALL: [Section; 29] = [
        Section::General,
        Section::Appearance,
        Section::Editor,
        Section::EditorAppearance,
        Section::EditorEditing,
        Section::EditorSaving,
        Section::EditorIntelligence,
        Section::EditorSearch,
        Section::EditorVim,
        Section::Languages,
        Section::Keymap,
        Section::Terminal,
        Section::Browser,
        Section::Git,
        Section::Worktrees,
        Section::Agents,
        Section::AgentServers,
        Section::McpServers,
        Section::ModelServers,
        Section::ModelSlots,
        Section::Orchestration,
        Section::Usage,
        Section::Customizations,
        Section::Skills,
        Section::Instructions,
        Section::AgentDefinitions,
        Section::Hooks,
        Section::Tools,
        Section::Servers,
    ];

    /// What the list and the heading call it.
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Languages => "Languages",
            Self::Terminal => "Terminal",
            Self::Browser => "Browser",
            Self::Git => "Git",
            Self::Worktrees => "Worktrees",
            Self::Servers => "Servers",
            Self::Results => "Search Results",
            Self::Editor => "Editor",
            Self::EditorAppearance => "Appearance",
            Self::EditorEditing => "Editing",
            Self::EditorSaving => "Saving",
            Self::EditorIntelligence => "Code Intelligence",
            Self::EditorSearch => "Search & Scrolling",
            Self::EditorVim => "Vim",
            Self::Keymap => "Keymap",
            Self::Agents => "Agents",
            Self::AgentServers => "Agents and accounts",
            Self::McpServers => "MCP Servers",
            Self::ModelServers => "Model Servers",
            Self::ModelSlots => "Models",
            Self::Orchestration => "Orchestration",
            Self::Usage => "Usage",
            Self::Customizations => "Overview",
            Self::Skills => "Skills",
            Self::Instructions => "Instructions",
            Self::AgentDefinitions => "Agents",
            Self::Hooks => "Hooks",
            Self::Tools => "Tools",
        }
    }

    /// The glyph before its name in the list.
    fn icon(self) -> Icon {
        match self {
            Self::General | Self::Results => phosphor::GEAR,
            Self::Appearance => phosphor::PENCIL_SIMPLE,
            Self::Languages => phosphor::FILE,
            Self::Terminal => phosphor::TERMINAL,
            Self::Browser => phosphor::GLOBE,
            Self::Git | Self::Worktrees => phosphor::GIT_BRANCH,
            Self::Servers => phosphor::PLUG,
            Self::Editor
            | Self::EditorAppearance
            | Self::EditorEditing
            | Self::EditorSaving
            | Self::EditorIntelligence
            | Self::EditorSearch
            | Self::EditorVim => phosphor::CODE,
            Self::Keymap => phosphor::COMMAND,
            Self::Agents => phosphor::SPARKLE,
            Self::AgentServers => phosphor::FOLDER,
            Self::McpServers => phosphor::PLUG,
            Self::ModelServers => phosphor::LIGHTNING,
            Self::ModelSlots => phosphor::SPARKLE,
            Self::Orchestration => phosphor::SLIDERS_HORIZONTAL,
            Self::Usage => phosphor::TIMER,
            Self::Customizations
            | Self::Skills
            | Self::Instructions
            | Self::AgentDefinitions
            | Self::Hooks
            | Self::Tools => phosphor::SPARKLE,
        }
    }

    /// The section it sits inside, if it sits inside one.
    pub(crate) fn parent(self) -> Option<Section> {
        match self {
            Self::AgentServers
            | Self::McpServers
            | Self::ModelServers
            | Self::ModelSlots
            | Self::Orchestration
            | Self::Usage
            | Self::Customizations
            | Self::Skills
            | Self::Instructions
            | Self::AgentDefinitions
            | Self::Hooks
            | Self::Tools => Some(Self::Agents),
            Self::EditorAppearance
            | Self::EditorEditing
            | Self::EditorSaving
            | Self::EditorIntelligence
            | Self::EditorSearch
            | Self::EditorVim => Some(Self::Editor),
            Self::Worktrees => Some(Self::Git),
            Self::General
            | Self::Appearance
            | Self::Languages
            | Self::Terminal
            | Self::Browser
            | Self::Git
            | Self::Servers
            | Self::Results
            | Self::Agents
            | Self::Editor
            | Self::Keymap => None,
        }
    }

    /// The name of pressing its row in the list.
    pub fn id(self) -> Target {
        let position = Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or_default();
        SECTIONS.at(position)
    }

    /// The section whose row in the list `id` names, when it names one.
    fn of(id: Target) -> Option<Self> {
        SECTIONS
            .index_of(id)
            .and_then(|index| Self::ALL.get(index).copied())
    }
}

/// Whether `id` names something of the window.
pub fn is_settings(id: Target) -> bool {
    matches!(id, Target::Settings(SettingsEvent::SettingsRow(..)))
        || matches!(
            id,
            Target::Settings(SettingsEvent::SlotsScopeUser)
                | Target::Settings(SettingsEvent::SlotsScopeProject)
                | Target::Settings(SettingsEvent::SlotNewName)
                | Target::Settings(SettingsEvent::SlotAdd)
                | Target::Settings(SettingsEvent::PresetName)
                | Target::Settings(SettingsEvent::PresetSave)
                | Target::Settings(SettingsEvent::SlotsProvider)
                | Target::Settings(SettingsEvent::CategoryNewName)
                | Target::Settings(SettingsEvent::CategoryAdd)
                | Target::Settings(SettingsEvent::PromptReplace)
                | Target::Settings(SettingsEvent::PromptAppend)
                | Target::Settings(SettingsEvent::DraftNew)
                | Target::Settings(SettingsEvent::DraftName)
                | Target::Settings(SettingsEvent::DraftDescription)
                | Target::Settings(SettingsEvent::DraftCreate)
                | Target::Settings(SettingsEvent::DraftCancel)
                | Target::Settings(SettingsEvent::SettingsClose)
                | Target::Settings(SettingsEvent::SettingsAdd)
                | Target::Settings(SettingsEvent::SettingsSave)
                | Target::Settings(SettingsEvent::SettingsCancel)
                | Target::Settings(SettingsEvent::SettingsName)
                | Target::Settings(SettingsEvent::SettingsTarget)
                | Target::Settings(SettingsEvent::SettingsEnv)
                | Target::Settings(SettingsEvent::SettingsSearch)
                | Target::Settings(SettingsEvent::SettingsSearchGo)
                | Target::Settings(SettingsEvent::SettingsToggleInstalled)
                | Target::Settings(SettingsEvent::SettingsToggleAvailable)
                | Target::Settings(SettingsEvent::SettingsMenuToggle)
                | Target::Settings(SettingsEvent::SettingsMenuEdit)
                | Target::Settings(SettingsEvent::SettingsMenuCopy)
                | Target::Settings(SettingsEvent::SettingsMenuWebsite)
                | Target::Settings(SettingsEvent::SettingsMenuRemove)
                | Target::Settings(SettingsEvent::SettingsDocs)
                | Target::Settings(SettingsEvent::SettingsFind)
                | Target::Settings(SettingsEvent::SettingsPickFilter)
                | Target::Settings(SettingsEvent::SettingsKeymapFind)
                | Target::Settings(SettingsEvent::SettingsFindClear)
                | Target::Settings(SettingsEvent::SettingsRelease)
                | Target::Settings(SettingsEvent::SettingsPickFirst)
                | Target::Settings(SettingsEvent::MachineAdd)
                | Target::Settings(SettingsEvent::MachineSave)
                | Target::Settings(SettingsEvent::MachineCancel)
                | Target::Settings(SettingsEvent::MachineName)
                | Target::Settings(SettingsEvent::MachineAddress)
                | Target::Settings(SettingsEvent::MachineToken)
                | Target::Settings(SettingsEvent::ModelAdd)
                | Target::Settings(SettingsEvent::ModelSave)
                | Target::Settings(SettingsEvent::ModelCancel)
                | Target::Settings(SettingsEvent::ModelName)
                | Target::Settings(SettingsEvent::ModelUrl)
                | Target::Settings(SettingsEvent::ModelKey)
                | Target::Settings(SettingsEvent::ModelKeyEnv)
                | Target::Settings(SettingsEvent::ModelList)
                | Target::Settings(SettingsEvent::ModelFind)
                | Target::Settings(SettingsEvent::ModelWireChat)
                | Target::Settings(SettingsEvent::ModelWireResponses)
                | Target::Settings(SettingsEvent::KeyField)
                | Target::Settings(SettingsEvent::KeySave)
                | Target::Settings(SettingsEvent::KeyCancel)
                | Target::Settings(SettingsEvent::NameField)
                | Target::Settings(SettingsEvent::NameSave)
                | Target::Settings(SettingsEvent::NameCancel)
        )
}

impl WorkspaceState {
    /// Whether the settings window is open.
    pub fn settings_open(&self) -> bool {
        self.settings.is_some()
    }

    /// The state with the settings window open on its first section.
    pub(crate) fn with_settings(mut self) -> Self {
        self.menu = None;
        self.picker = None;
        self.settings = Some(Section::default());
        self.settings_servers = settings_servers::ServerSettings::opening(
            std::mem::take(&mut self.settings_servers.agents),
            std::mem::take(&mut self.settings_servers.retired),
        );
        self.settings_page.picker = None;
        self.settings_page.fields.clear();
        self.fill_fields();
        self.outbox.push(crate::state::Outgoing::LoadServers);
        self.outbox.push(crate::state::Outgoing::ModelServers);
        self.outbox.push(crate::state::Outgoing::Extensions);
        self.read_library();
        if self.window.general.check_updates && self.settings_page.release.is_none() {
            self.outbox.push(crate::state::Outgoing::LatestRelease {
                channel: self.window.general.channel,
            });
        }
        self
    }

    /// The state with settings closed, whatever form they had open.
    pub fn without_settings(mut self) -> Self {
        self.close_settings();
        self
    }

    /// Closes settings, whatever form they had open.
    pub(crate) fn close_settings(&mut self) {
        self.settings = None;
        self.settings_servers.form = None;
        self.settings_servers.key_form = None;
        self.settings_servers.name_form = None;
        self.settings_page.server_form = None;
        self.settings_page.models.form = None;
        self.settings_page.slots.picking = None;
        self.settings_page.library.draft = None;
        self.settings_page.picker = None;
        self.recording = None;
    }

    /// The state after something of the settings window was pressed: `action` is the name of what.
    pub(crate) fn settings_pressed(mut self, action: Target) -> Self {
        match action {
            Target::Settings(SettingsEvent::SettingsClose)
                if self.settings_page.picker.is_some() =>
            {
                self.settings_page.picker = None;
            }
            Target::Settings(SettingsEvent::SettingsClose)
                if self.settings_page.slots.picking.is_some() =>
            {
                self.settings_page.slots.picking = None;
            }
            Target::Settings(SettingsEvent::SettingsClose)
                if self.settings_servers.form.is_some() =>
            {
                self.settings_servers.form = None;
            }
            Target::Settings(SettingsEvent::SettingsClose)
                if self.settings_servers.key_form.is_some() =>
            {
                self.settings_servers.key_form = None;
            }
            Target::Settings(SettingsEvent::SettingsClose)
                if self.settings_servers.name_form.is_some() =>
            {
                self.settings_servers.name_form = None;
            }
            Target::Settings(SettingsEvent::SettingsClose)
                if self.settings_page.models.form.is_some() =>
            {
                self.settings_page.models.form = None;
            }
            Target::Settings(SettingsEvent::SettingsClose)
                if self.settings_page.server_form.is_some() =>
            {
                self.settings_page.server_form = None;
            }
            Target::Settings(SettingsEvent::SettingsClose)
                if self.settings_page.library.draft.is_some() =>
            {
                self.settings_page.library.draft = None;
            }
            Target::Settings(SettingsEvent::SettingsClose) => return self.without_settings(),
            Target::Settings(SettingsEvent::SettingsRelease) => {
                self.open_release();
                return self.without_settings();
            }
            Target::Settings(SettingsEvent::SettingsFindClear) => {
                self.settings_page.find = rustle::TextFieldState::default();
                self.settings_servers.scroll.jump_to(0.0);
            }
            action if self.slots_pressed(action) => {}
            action if self.setting_pressed(action) => {}
            action if self.keymap_pressed(action) => {}
            action if self.machines_pressed(action) => {}
            action if self.models_pressed(action) => {}
            action if self.usage_pressed(action) => {}
            action if self.languages_pressed(action) => {}
            action
                if self.draft_pressed(
                    action,
                    self.settings.and_then(crate::settings_library::kind_of),
                ) => {}
            action if self.prompt_pressed(action) => {}
            action if self.library_pressed(action) => {}
            action if self.agents_pressed(action) => {}
            action => match Section::of(action) {
                Some(section) => self.section_pressed(section),
                None => return self.servers_pressed(action),
            },
        }
        self
    }

    /// Shows `section`, whose row in the list was pressed; pressing Agents while it shows folds the sections inside it away or shows them.
    fn section_pressed(&mut self, section: Section) {
        if section == Section::Agents {
            let showing = self.settings == Some(section);
            self.settings_servers.folded = showing && !self.settings_servers.folded;
        }
        self.settings = Some(section);
        self.settings_page.find = rustle::TextFieldState::default();
        self.settings_page.picker = None;
        self.settings_page.slots.picking = None;
        self.recording = None;
        self.section_changed();
        if section == Section::ModelServers {
            self.outbox.push(crate::state::Outgoing::ModelServers);
        }
        if section == Section::Usage {
            self.ask_usage();
        }
        if matches!(section, Section::ModelSlots | Section::Orchestration) {
            self.ask_model_slots();
        }
        if section == Section::AgentServers {
            self.ask_default_accounts();
            self.ask_login_identities();
        }
        if crate::settings_library::lists(section) {
            self.read_library();
        }
    }

    /// Whether settings are being searched, which shows what was found in place of the section picked.
    pub(crate) fn settings_searching(&self) -> bool {
        !self.settings_page.find.text().trim().is_empty()
    }
}

/// How wide the list of sections is in a window `width` logical pixels across.
fn navigation_width(width: f32) -> f32 {
    if width <= NARROW_WINDOW {
        NARROW_NAVIGATION
    } else {
        NAVIGATION_WIDTH
    }
}

/// Emits what fills the settings window, `width` logical pixels across, when settings are open.
///
/// Returns whether settings are open, and so whether anything was emitted.
#[track_caller]
pub fn settings(state: &WorkspaceState, width: f32) -> bool {
    let Some(section) = state.settings else {
        return false;
    };
    let folded = state.settings_servers.folded;
    let navigation_width = navigation_width(width);
    let showing = if state.settings_searching() {
        Section::Results
    } else {
        section
    };
    kit::row_top(
        Modifier::new()
            .fill_max_size()
            .background(theme::color(Role::Surface)),
        0,
        || {
            navigation(state, section, folded, navigation_width);
            content(state, showing, width - navigation_width - 2.0 * PAGE_INSET);
        },
    );
    true
}

/// Emits the search box over the list of sections, and the list, with the one showing marked, `width` logical pixels across.
#[track_caller]
fn navigation(state: &WorkspaceState, showing: Section, folded: bool, width: f32) {
    let searching = state.settings_searching();
    kit::column(
        Modifier::new()
            .fill_max_height()
            .width(width)
            .ruled(theme::Side::Right)
            .padding(theme::sp(3)),
        1,
        || {
            kit::row(
                Modifier::new()
                    .height(theme::sp(9))
                    .padding_xy(theme::sp(2), 0.0),
                0,
                || {
                    line("Settings", 13, 1.4, Role::Subtle);
                },
            );
            find(state, searching);
            kit::column(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .vertical_scroll(&state.settings_page.nav_scroll)
                    .fill_max_width(),
                1,
                || {
                    for section in
                        Section::ALL
                            .into_iter()
                            .filter(|section| match section.parent() {
                                Some(Section::Agents) => !folded,
                                Some(parent) => {
                                    showing == parent || showing.parent() == Some(parent)
                                }
                                None => true,
                            })
                    {
                        entry(
                            section,
                            !searching && section == showing,
                            folded,
                            state.library_count(section),
                        );
                    }
                },
            );
        },
    );
}

/// Emits the box searching settings, with the button clearing it while `searching`.
#[track_caller]
fn find(state: &WorkspaceState, searching: bool) {
    kit::row(
        Modifier::new()
            .fill_max_width()
            .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
        1,
        || {
            Input::search(
                &state.settings_page.find,
                Target::Settings(SettingsEvent::SettingsFind),
            )
            .size(Size::Sm)
            .placeholder("Search settings")
            .modifier(Modifier::new().weight(1.0))
            .build();
            if searching {
                row_arranged(
                    Modifier::new()
                        .size(theme::sp(6), theme::sp(6))
                        .indicated(
                            theme::InteractionTones::new()
                                .hover(Role::SurfaceHover)
                                .radius(Radius::Sm),
                            theme::Focused::Held(crate::focus::is_held(Target::Settings(
                                SettingsEvent::SettingsFindClear,
                            ))),
                        )
                        .cursor(CursorIcon::Pointer)
                        .control(
                            Target::Settings(SettingsEvent::SettingsFindClear),
                            send(Event::Settings(SettingsEvent::SettingsFindClear)),
                        ),
                    Arrangement::Center,
                    Alignment::Center,
                    || glyph(phosphor::X, 3.5, Role::Muted),
                );
            }
        },
    );
}

/// Emits one row of the list of sections: a section of its own has its glyph, Agents the chevron that says whether the sections inside it are shown, and those inside another only their name, set in under it, with `count` after it when it has one.
#[track_caller]
fn entry(section: Section, showing: bool, folded: bool, count: Option<usize>) {
    let tone = if showing { Role::Text } else { Role::Muted };
    let nested = section != Section::Agents && section.parent().is_some();
    let left = if nested { 8.0 } else { 2.0 };
    Tile::new(section.id())
        .selected(showing)
        .modifier(Modifier::new().fill_max_width())
        .inner(
            Modifier::new()
                .padding_each(theme::sp(left), 0.0, 0.0, 0.0)
                .padding_each(0.0, 0.0, theme::sp(2), 0.0),
        )
        .build_with(|| {
            match section {
                Section::Agents => glyph(
                    if folded {
                        phosphor::CARET_RIGHT
                    } else {
                        phosphor::CARET_DOWN
                    },
                    4.0,
                    tone,
                ),
                _ if nested => {}
                _ => glyph(section.icon(), 4.0, tone),
            }
            clipped(section.title(), 13, tone);
            if let Some(count) = count {
                crate::Badge::new(count.to_string()).build();
            }
        });
}

/// Emits the settings of `section` under its title, with `room` logical pixels across for them.
#[track_caller]
fn content(state: &WorkspaceState, section: Section, room: f32) {
    kit::column(
        Modifier::new()
            .fill_max_height()
            .weight(1.0)
            .background(theme::color(Role::Background)),
        0,
        || {
            kit::row(
                Modifier::new()
                    .padding_each(theme::sp(8), 0.0, 0.0, 0.0)
                    .padding_each(0.0, 0.0, theme::sp(8), 0.0)
                    .padding_each(0.0, theme::sp(6), 0.0, 0.0)
                    .padding_each(0.0, 0.0, 0.0, theme::sp(4)),
                0,
                || settings_servers::page_title(section),
            );
            settings_servers::body(state, section, room);
        },
    );
}

#[cfg(test)]
/// Tests for what pressing the controls of the settings window does.
mod tests {
    use pm_config::KnownServer;
    use pm_protocol::library::Kind;

    use super::*;
    use crate::settings_catalog::{LISTS, OPEN_LIST, Setting};
    use crate::settings_rows::{KEY_SETS, LIST_ADDS, LIST_FIELDS, PICKS};
    use crate::settings_values::Control;
    use crate::state::Outgoing;

    /// The state with the settings window open on `section`, a server known besides this machine, and nothing waiting to be sent.
    fn open_on(section: Section) -> WorkspaceState {
        let mut state = WorkspaceState::new(&[]).with_settings();
        state.window.servers.known.push(KnownServer {
            name: "Lab".to_owned(),
            address: "100.64.0.9:6868".to_owned(),
            ..KnownServer::default()
        });
        let mut state = state.settings_pressed(section.id());
        state.take_outbox();
        state
    }

    /// The state with the list of interface fonts open over 2,000 installed families, narrowed to `needle`.
    fn fonts_narrowed_to(needle: &str) -> WorkspaceState {
        let families: Vec<String> = (0..2000)
            .map(|index| format!("Family {index:04}"))
            .collect();
        let state = open_on(Section::Appearance)
            .with_fonts(families, Vec::new())
            .settings_pressed(Setting::InterfaceFont.id(OPEN_LIST).unwrap());
        let mut state = state.typed(Target::Settings(SettingsEvent::SettingsPickFilter), needle);
        state.take_outbox();
        state
    }

    /// A font far down a long list is picked by its row among those shown, and pressing it reaches no other control: no machine, key, sign-in or language is touched.
    #[test]
    fn a_font_far_down_the_list_is_picked_and_nothing_else() {
        let mut state = fonts_narrowed_to("Family 1500").settings_pressed(PICKS.at(0));
        assert_eq!(state.window.appearance.interface_font, "Family 1500");
        assert_eq!(state.settings_page.picker, None);
        assert_eq!(state.window.servers.known.len(), 1);
        assert!(state.settings_page.server_form.is_none());
        assert!(state.settings_servers.key_form.is_none());
        let sent = state.take_outbox();
        assert!(
            !sent.iter().any(|outgoing| matches!(
                outgoing,
                Outgoing::SaveKey { .. }
                    | Outgoing::SignIn { .. }
                    | Outgoing::InstallExtensions { .. }
                    | Outgoing::RemoveExtension { .. }
                    | Outgoing::RemovePackage { .. }
            )),
            "{sent:?}"
        );
    }

    /// Enter in the box narrowing an open list picks the first choice it shows.
    #[test]
    fn enter_in_the_filter_picks_the_first_match() {
        let state = fonts_narrowed_to("family 0042");
        let submit = WorkspaceState::submit_of(Target::Settings(SettingsEvent::SettingsPickFilter));
        assert_eq!(submit, Target::Settings(SettingsEvent::SettingsPickFirst));
        let state = state.settings_pressed(submit);
        assert_eq!(state.window.appearance.interface_font, "Family 0042");
        assert_eq!(state.settings_page.picker, None);
    }

    /// A line added from the Editor overview goes into the list whose box it was typed in, not into the list of whatever page the window guessed.
    #[test]
    fn a_list_on_the_overview_adds_to_its_own_setting() {
        let binding = "g h => Hover";
        assert!(crate::vim::binding_reads(binding));
        let place = Setting::VimBindings.list_place().unwrap();
        let field = LIST_FIELDS.at(place);
        let include = open_on(Section::Editor).machine.git.worktree_include;
        let state = open_on(Section::Editor).typed(field, binding);
        let add = WorkspaceState::submit_of(field);
        assert_eq!(add, LIST_ADDS.at(place));
        let state = state.settings_pressed(add);
        assert!(state.editor.vim_bindings.iter().any(|held| held == binding));
        assert_eq!(state.machine.git.worktree_include, include);
        assert!(
            state
                .text_field(field)
                .is_some_and(|field| field.text().is_empty())
        );
    }

    /// Escape on the form for a new customization puts the form away and keeps the window open.
    #[test]
    fn escape_on_the_draft_form_closes_only_the_form() {
        let state =
            open_on(Section::Skills).settings_pressed(Target::Settings(SettingsEvent::DraftNew));
        assert!(state.settings_page.library.draft.is_some());
        let state = state.settings_pressed(Target::Settings(SettingsEvent::SettingsClose));
        assert!(state.settings_page.library.draft.is_none());
        assert_eq!(state.settings, Some(Section::Skills));
        let state = state
            .settings_pressed(Target::Settings(SettingsEvent::DraftNew))
            .without_settings();
        assert!(state.settings_page.library.draft.is_none());
    }

    /// Changing section puts away the box for an API key and the form of the servers page.
    #[test]
    fn changing_section_puts_forms_away() {
        let state = open_on(Section::Servers)
            .settings_pressed(Target::Settings(SettingsEvent::MachineAdd))
            .settings_pressed(Section::AgentServers.id())
            .settings_pressed(KEY_SETS.at(0));
        assert!(state.settings_page.server_form.is_none());
        assert!(state.settings_servers.key_form.is_some());
        let state = state.settings_pressed(Section::General.id());
        assert!(state.settings_servers.key_form.is_none());
    }

    /// The button setting the key of the thirty-first agent opens its box, rather than forgetting the first agent's key.
    #[test]
    fn a_late_agents_key_button_is_its_own() {
        let mut state = open_on(Section::AgentServers).settings_pressed(KEY_SETS.at(30));
        assert!(state.settings_servers.key_form.is_some());
        assert!(
            !state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::SaveKey { .. }))
        );
    }

    /// A customization written while the form was put away still has the pages read again and its file opened.
    #[test]
    fn a_customization_written_after_its_form_closed_still_opens() {
        let mut state = open_on(Section::Skills);
        state.open_draft(Kind::Skill);
        let mut state = state
            .settings_pressed(Section::Hooks.id())
            .with_customization_created(None, "/home/me/.claude/skills/x/SKILL.md".to_owned());
        let sent = state.take_outbox();
        assert!(
            sent.iter()
                .any(|outgoing| matches!(outgoing, Outgoing::Library { .. }))
        );
        assert!(sent.iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::OpenPath { path } if path.ends_with("SKILL.md")
        )));
    }

    /// Every row of choices fits before the controls opening its list and resetting it, and every setting drawn as a list has its own box and buttons.
    #[test]
    fn choices_and_lists_fit_their_controls() {
        let state = WorkspaceState::new(&[]);
        for setting in Setting::ALL {
            match setting.control(&state) {
                Control::Choice(names, _) => {
                    assert!(names.len() <= OPEN_LIST, "{setting:?}");
                }
                Control::List(_) => assert!(LISTS.contains(&setting), "{setting:?}"),
                _ => {}
            }
        }
    }
}
