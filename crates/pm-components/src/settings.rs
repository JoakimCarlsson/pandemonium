//! The settings window: the sections of settings down its left side and the one chosen on its right, filling a window of its own.

use crate::areas::SettingsEvent;
use crate::events;
use crate::icons;
use crate::theme::Indicated;

use crate::target::Target;
use crate::theme;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{Icon, phosphor};

use crate::theme::{Radius, Role};

use crate::input::Input;
use crate::parts::{clipped, glyph, line};
use crate::settings_editor::PAGES;
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

/// The pages inside Agents that are not lists of what agents are given, in the order the list and the overview show them.
pub(crate) const AGENT_PAGES: [Section; 6] = [
    Section::AgentServers,
    Section::McpServers,
    Section::ModelServers,
    Section::ModelSlots,
    Section::Orchestration,
    Section::Usage,
];

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
            Self::Appearance | Self::EditorAppearance => "Appearance",
            Self::Languages => "Languages",
            Self::Terminal => "Terminal",
            Self::Browser => "Browser",
            Self::Git => "Git",
            Self::Worktrees => "Worktrees",
            Self::Servers => "Servers",
            Self::Results => "Search Results",
            Self::Editor => "Editor",
            Self::EditorEditing => "Editing",
            Self::EditorSaving => "Saving",
            Self::EditorIntelligence => "Code Intelligence",
            Self::EditorSearch => "Search & Scrolling",
            Self::EditorVim => "Vim",
            Self::Keymap => "Keymap",
            Self::Agents | Self::AgentDefinitions => "Agents",
            Self::AgentServers => "Agents and accounts",
            Self::McpServers => "MCP Servers",
            Self::ModelServers => "Model Servers",
            Self::ModelSlots => "Models",
            Self::Orchestration => "Orchestration",
            Self::Usage => "Usage",
            Self::Customizations => "Overview",
            Self::Skills => "Skills",
            Self::Instructions => "Instructions",
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
            Self::Servers | Self::McpServers => phosphor::PLUG,
            Self::Keymap => phosphor::COMMAND,
            Self::AgentServers => phosphor::FOLDER,
            Self::ModelServers => phosphor::LIGHTNING,
            Self::Orchestration => phosphor::SLIDERS_HORIZONTAL,
            Self::Usage => phosphor::TIMER,
            _ if self == Self::Editor || PAGES.contains(&self) => phosphor::CODE,
            _ => phosphor::SPARKLE,
        }
    }

    /// The section it sits inside, if it sits inside one.
    pub(crate) fn parent(self) -> Option<Section> {
        match self {
            Self::Worktrees => Some(Self::Git),
            _ if PAGES.contains(&self) => Some(Self::Editor),
            _ if AGENT_PAGES.contains(&self) || crate::settings_library::lists(self) => {
                Some(Self::Agents)
            }
            _ => None,
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

/// Whether `id` names something of the window: anything of the settings area but the extensions page and the previews it opens.
pub fn is_settings(id: Target) -> bool {
    matches!(
        id,
        Target::Settings(event) if !matches!(
            event,
            SettingsEvent::Extensions
                | SettingsEvent::ExtensionsSearch
                | SettingsEvent::InstallExtension
                | SettingsEvent::PreviewCancel
                | SettingsEvent::PreviewInstall
                | SettingsEvent::PreviewGrant(..)
                | SettingsEvent::ExtensionRow(..)
                | SettingsEvent::ExtensionGrant { .. }
        )
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
        while self.put_away_innermost() {}
        self.recording = None;
    }

    /// The state after something of the settings window was pressed: `action` is the name of what.
    pub(crate) fn settings_pressed(mut self, action: Target) -> Self {
        match action {
            Target::Settings(SettingsEvent::SettingsClose) => {
                if !self.put_away_innermost() {
                    return self.without_settings();
                }
            }
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

    /// Puts away the list, picker or form open over the page, the first of them open in the order Escape closes them; says whether one was open.
    pub(crate) fn put_away_innermost(&mut self) -> bool {
        let (page, servers) = (&mut self.settings_page, &mut self.settings_servers);
        page.picker.take().is_some()
            || page.slots.picking.take().is_some()
            || servers.form.take().is_some()
            || servers.key_form.take().is_some()
            || servers.name_form.take().is_some()
            || page.models.form.take().is_some()
            || page.server_form.take().is_some()
            || page.library.draft.take().is_some()
    }

    /// Shows `section`, whose row in the list was pressed; pressing Agents while it shows folds the sections inside it away or shows them.
    fn section_pressed(&mut self, section: Section) {
        if section == Section::Agents {
            let showing = self.settings == Some(section);
            self.settings_servers.folded = showing && !self.settings_servers.folded;
        }
        self.settings = Some(section);
        self.settings_page.find = rustle::TextFieldState::default();
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

/// Emits what fills the settings window, `width` logical pixels across, when settings are open.
///
/// Returns whether settings are open, and so whether anything was emitted.
#[track_caller]
pub fn settings(state: &WorkspaceState, width: f32) -> bool {
    let Some(section) = state.settings else {
        return false;
    };
    let folded = state.settings_servers.folded;
    let navigation_width = if width <= NARROW_WINDOW {
        NARROW_NAVIGATION
    } else {
        NAVIGATION_WIDTH
    };
    let showing = if state.settings_searching() {
        Section::Results
    } else {
        section
    };
    rustle::row_arranged(
        Modifier::new()
            .fill_max_size()
            .background(theme::color(Role::Surface)),
        Arrangement::Start,
        Alignment::Start,
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
    rustle::column_arranged(
        Modifier::new()
            .fill_max_height()
            .width(width)
            .border_right_hairline(theme::color(Role::Border))
            .padding_each(0.0, 0.0, 1.0, 0.0)
            .padding(theme::sp(3)),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new()
                    .height(theme::sp(9))
                    .padding_xy(theme::sp(2), 0.0),
                Arrangement::Start,
                Alignment::Center,
                || {
                    line("Settings", 13, 1.4, Role::Subtle);
                },
            );
            find(state, searching);
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .vertical_scroll(&state.settings_page.nav_scroll)
                    .fill_max_width(),
                Arrangement::SpacedBy(theme::sp(1)),
                Alignment::Start,
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
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            Input::search(
                &state.settings_page.find,
                Target::Settings(SettingsEvent::SettingsFind),
            )
            .size(Size::Sm)
            .placeholder("Search settings")
            .modifier(Modifier::new().weight(1.0))
            .submits(Target::Settings(SettingsEvent::SettingsFind))
            .build();
            if searching {
                let clear = Target::Settings(SettingsEvent::SettingsFindClear);
                let own = Modifier::new().size(theme::sp(6), theme::sp(6)).indicated(
                    theme::InteractionTones::new()
                        .hover(Role::SurfaceHover)
                        .radius(Radius::Sm),
                    theme::Focused::Control(crate::focus::is_focused(clear)),
                );
                row_arranged(
                    events::pressable(own, Some(clear)),
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
        .inner(Modifier::new().padding_each(theme::sp(left), 0.0, theme::sp(2), 0.0))
        .build_with(|| {
            match section {
                Section::Agents => glyph(icons::chevron(!folded), 4.0, tone),
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
    rustle::column_arranged(
        Modifier::new()
            .fill_max_height()
            .weight(1.0)
            .background(theme::color(Role::Background)),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().padding_each(
                    theme::sp(8),
                    theme::sp(6),
                    theme::sp(8),
                    theme::sp(4),
                ),
                Arrangement::Start,
                Alignment::Center,
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
        let state = state.settings_pressed(Target::Settings(SettingsEvent::SettingsPickFirst));
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
        let add = LIST_ADDS.at(place);
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
