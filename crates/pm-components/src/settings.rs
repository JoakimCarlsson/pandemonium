//! The settings window: the sections of settings down its left side and the one chosen on its right, filling a window of its own.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, bg, border_color, border_r, flex, flex_1, flex_col, flex_none, gap, h, h_full,
    items_center, min_h, min_w_px, overflow_hidden, p, pb, pl, pr, pt, px, w_full, w_px,
};
use pm_ui::{Node, node};

use crate::input::Input;
use crate::parts::{clipped, glyph, line};
use crate::settings_servers;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::tile::Tile;

/// The first name the settings window answers to.
const SETTINGS_START: u64 = 20_000_000;

/// Closes the settings window.
pub const SETTINGS_CLOSE: FocusId = FocusId(20_000_002);

/// Opens the form for adding a server to the list showing.
pub const SETTINGS_ADD: FocusId = FocusId(20_000_003);

/// Keeps what the form holds.
pub const SETTINGS_SAVE: FocusId = FocusId(20_000_004);

/// Puts the form away without keeping it.
pub const SETTINGS_CANCEL: FocusId = FocusId(20_000_005);

/// The box the form's name is written in.
pub const SETTINGS_NAME: FocusId = FocusId(20_000_006);

/// The box the form's command or address is written in.
pub const SETTINGS_TARGET: FocusId = FocusId(20_000_007);

/// The box the form's environment is written in.
pub const SETTINGS_ENV: FocusId = FocusId(20_000_008);

/// The box the form's package, which the runner fetches when the agent is not installed, is written in.
pub const SETTINGS_PACKAGE: FocusId = FocusId(20_000_020);

/// The box the form's install command is written in.
pub const SETTINGS_INSTALLER: FocusId = FocusId(20_000_021);

/// The box the form's login folder variable is written in.
pub const SETTINGS_HOME: FocusId = FocusId(20_000_022);

/// The box the list showing is searched with.
pub const SETTINGS_SEARCH: FocusId = FocusId(20_000_030);

/// Searches the registry for the words in the search box, which is what Enter in the box does.
pub const SETTINGS_SEARCH_GO: FocusId = FocusId(20_000_031);

/// The scrolling page of the section showing.
pub const SETTINGS_SCROLL: FocusId = FocusId(20_000_040);

/// The scrolling list of sections down the left side.
pub const SETTINGS_NAV_SCROLL: FocusId = FocusId(20_000_041);

/// Folds the list of what is installed away, or shows it again.
pub const SETTINGS_TOGGLE_INSTALLED: FocusId = FocusId(20_000_032);

/// Folds the list of what the registry offers away, or shows it again.
pub const SETTINGS_TOGGLE_AVAILABLE: FocusId = FocusId(20_000_033);

/// Switches the server the open menu is over on or off.
pub const SETTINGS_MENU_TOGGLE: FocusId = FocusId(20_000_050);

/// Opens the form for the server the open menu is over.
pub const SETTINGS_MENU_EDIT: FocusId = FocusId(20_000_051);

/// Copies the configuration of the server the open menu is over.
pub const SETTINGS_MENU_COPY: FocusId = FocusId(20_000_052);

/// Opens the page of the publisher of the server the open menu is over.
pub const SETTINGS_MENU_WEBSITE: FocusId = FocusId(20_000_053);

/// Takes the server the open menu is over away.
pub const SETTINGS_MENU_REMOVE: FocusId = FocusId(20_000_054);

/// Opens the page that explains tool servers.
pub const SETTINGS_DOCS: FocusId = FocusId(20_000_055);

/// The first of the switches turning a server on or off; the rest follow it.
pub const SETTINGS_SWITCH_BASE: u64 = 20_000_500;

/// The first of the buttons opening the menu of a server; the rest follow it.
pub const SETTINGS_MORE_BASE: u64 = 20_000_600;

/// The first of the buttons removing an account of an agent; the rest follow it.
pub const SETTINGS_ACCOUNT_REMOVE_BASE: u64 = 20_000_700;

/// The first of the buttons adding an account to an agent; the rest follow it.
pub const SETTINGS_ACCOUNT_ADD_BASE: u64 = 20_000_800;

/// The first of the sections down the left side; the rest follow it.
pub const SETTINGS_SECTION_BASE: u64 = 20_000_900;

/// The first of the buttons editing a server of the list showing; the rest follow it.
pub const SETTINGS_EDIT_BASE: u64 = 20_000_100;

/// The first of the buttons removing a server of the list showing; the rest follow it.
pub const SETTINGS_REMOVE_BASE: u64 = 20_000_200;

/// The first of the buttons installing what a registry offers; the rest follow it.
pub const SETTINGS_INSTALL_BASE: u64 = 20_000_300;

/// The box every setting is searched with.
pub const SETTINGS_FIND: FocusId = FocusId(20_000_060);

/// The box the open list of choices is narrowed with.
pub const SETTINGS_PICK_FILTER: FocusId = FocusId(20_000_061);

/// The box the keymap's actions are searched with.
pub const SETTINGS_KEYMAP_FIND: FocusId = FocusId(20_000_062);

/// Empties the box every setting is searched with.
pub const SETTINGS_FIND_CLEAR: FocusId = FocusId(20_000_063);

/// Opens the page of the newest release.
pub const SETTINGS_RELEASE: FocusId = FocusId(20_000_064);

/// One past the last name the window answers to.
const SETTINGS_END: u64 = 20_010_000;

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
    pub const ALL: [Section; 25] = [
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
            Self::AgentServers => "Agent Servers",
            Self::McpServers => "MCP Servers",
            Self::Customizations => "Overview",
            Self::Skills => "Skills",
            Self::Instructions => "Instructions",
            Self::AgentDefinitions => "Agents",
            Self::Hooks => "Hooks",
            Self::Tools => "Tools",
        }
    }

    /// The glyph before its name in the list.
    fn icon(self) -> IconName {
        match self {
            Self::General | Self::Results => IconName::Settings,
            Self::Appearance => IconName::Pencil,
            Self::Languages => IconName::File,
            Self::Terminal => IconName::Terminal,
            Self::Browser => IconName::Globe,
            Self::Git | Self::Worktrees => IconName::Git,
            Self::Servers => IconName::Plug,
            Self::Editor
            | Self::EditorAppearance
            | Self::EditorEditing
            | Self::EditorSaving
            | Self::EditorIntelligence
            | Self::EditorSearch
            | Self::EditorVim => IconName::FileCode,
            Self::Keymap => IconName::SquareSlash,
            Self::Agents => IconName::Sparkles,
            Self::AgentServers => IconName::Folder,
            Self::McpServers => IconName::Plug,
            Self::Customizations
            | Self::Skills
            | Self::Instructions
            | Self::AgentDefinitions
            | Self::Hooks
            | Self::Tools => IconName::Sparkles,
        }
    }

    /// The section it sits inside, if it sits inside one.
    pub(crate) fn parent(self) -> Option<Section> {
        match self {
            Self::AgentServers
            | Self::McpServers
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
    pub fn id(self) -> FocusId {
        let position = Self::ALL.iter().position(|section| *section == self);
        FocusId(SETTINGS_SECTION_BASE + position.unwrap_or_default() as u64)
    }
}

/// Whether `id` names something of the window.
pub fn is_settings(id: FocusId) -> bool {
    let FocusId(number) = id;
    (SETTINGS_START..SETTINGS_END).contains(&number)
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
        self.settings_page.server_form = None;
        self.settings_page.picker = None;
        self.recording = None;
    }

    /// The state after something of the settings window was pressed: `action` is the name of what.
    pub(crate) fn settings_pressed(mut self, action: FocusId) -> Self {
        match action {
            SETTINGS_CLOSE if self.settings_page.picker.is_some() => {
                self.settings_page.picker = None;
            }
            SETTINGS_CLOSE if self.settings_servers.form.is_some() => {
                self.settings_servers.form = None;
            }
            SETTINGS_CLOSE if self.settings_page.server_form.is_some() => {
                self.settings_page.server_form = None;
            }
            SETTINGS_CLOSE => return self.without_settings(),
            SETTINGS_RELEASE => {
                self.open_release();
                return self.without_settings();
            }
            SETTINGS_FIND_CLEAR => {
                self.settings_page.find = pm_text::Field::default();
                self.settings_servers.scroll = 0.0;
            }
            action if self.setting_pressed(action) => {}
            action if self.keymap_pressed(action) => {}
            action if self.machines_pressed(action) => {}
            action if self.languages_pressed(action) => {}
            action if self.library_pressed(action) => {}
            action => {
                if let Some(section) = Section::ALL
                    .into_iter()
                    .find(|section| section.id() == action)
                {
                    if section == Section::Agents {
                        let showing = self.settings == Some(section);
                        self.settings_servers.folded = showing && !self.settings_servers.folded;
                    }
                    self.settings = Some(section);
                    self.settings_page.find = pm_text::Field::default();
                    self.settings_page.picker = None;
                    self.recording = None;
                    self.section_changed();
                    if section == Section::AgentServers {
                        self.ask_default_accounts();
                    }
                    if crate::settings_library::lists(section) {
                        self.read_library();
                    }
                } else {
                    return self.servers_pressed(action);
                }
            }
        }
        self
    }

    /// The state after the list of sections is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_settings_nav(mut self, delta: f32, room: f32) -> Self {
        self.settings_page.nav_scroll = (self.settings_page.nav_scroll + delta).clamp(0.0, room);
        self
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

/// The node that fills the settings window, `width` logical pixels across, when settings are open.
pub fn settings(state: &WorkspaceState, width: f32) -> Option<Node> {
    let section = state.settings?;
    let folded = state.settings_servers.folded;
    let navigation_width = navigation_width(width);
    let showing = if state.settings_searching() {
        Section::Results
    } else {
        section
    };
    Some(
        node()
            .styles([flex(), w_full(), h_full(), min_h(0), bg(Role::Surface)])
            .child(navigation(state, section, folded, navigation_width))
            .child(content(
                state,
                showing,
                width - navigation_width - 2.0 * PAGE_INSET,
            )),
    )
}

/// The search box over the list of sections, and the list, with the one showing marked, `width` logical pixels across.
fn navigation(state: &WorkspaceState, showing: Section, folded: bool, width: f32) -> Node {
    let searching = state.settings_searching();
    let find = node()
        .styles([flex(), items_center(), gap(1), flex_none(), pb(2)])
        .child(
            node().styles([flex_1(), min_w_px(0)]).child(
                Input::search(&state.settings_page.find, SETTINGS_FIND)
                    .size(Size::Sm)
                    .placeholder("Search settings")
                    .build(),
            ),
        )
        .children(searching.then(|| {
            node()
                .styles([
                    flex(),
                    items_center(),
                    pm_style::justify_center(),
                    flex_none(),
                    pm_style::size(6),
                    pm_style::rounded(pm_style::Radius::Sm),
                    pm_style::cursor_pointer(),
                    bg(Role::SurfaceHover).hover(),
                ])
                .focusable(SETTINGS_FIND_CLEAR)
                .child(glyph(IconName::Close, 3.5, Role::Muted))
        }));
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            gap(1),
            p(3),
            w_px(width),
            min_h(0),
            border_r(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([flex(), items_center(), h(9), px(2)])
                .child(line("Settings", 13, 1.4, Role::Subtle)),
        )
        .child(find)
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
                .scroll_from_start(SETTINGS_NAV_SCROLL, state.settings_page.nav_scroll)
                .child(
                    node()
                        .styles([flex(), flex_col(), flex_none(), gap(1)])
                        .children(
                            Section::ALL
                                .into_iter()
                                .filter(|section| match section.parent() {
                                    Some(Section::Agents) => !folded,
                                    Some(parent) => {
                                        showing == parent || showing.parent() == Some(parent)
                                    }
                                    None => true,
                                })
                                .map(|section| {
                                    entry(
                                        section,
                                        !searching && section == showing,
                                        folded,
                                        state.library_count(section),
                                    )
                                }),
                        ),
                ),
        )
}

/// One row of the list of sections: a section of its own has its glyph, Agents the chevron that says whether the sections inside it are shown, and those inside another only their name, set in under it, with `count` after it when it has one.
fn entry(section: Section, showing: bool, folded: bool, count: Option<usize>) -> Node {
    let tone = if showing { Role::Text } else { Role::Muted };
    let row = Tile::new(section.id())
        .selected(showing)
        .build()
        .styles([px(2)]);
    let row = match section {
        Section::Agents => row.child(glyph(
            if folded {
                IconName::ChevronRight
            } else {
                IconName::ChevronDown
            },
            4.0,
            tone,
        )),
        _ if section.parent().is_some() => row.styles([pl(8)]),
        _ => row.child(glyph(section.icon(), 4.0, tone)),
    };
    row.child(clipped(section.title(), 13, tone).styles([pm_style::flex_1()]))
        .children(count.map(|count| crate::Badge::new(count.to_string()).build()))
}

/// The settings of `section` under its title, with `room` logical pixels across for them.
fn content(state: &WorkspaceState, section: Section, room: f32) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_1(),
            min_w_px(0),
            bg(Role::Background),
        ])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    pl(8),
                    pr(8),
                    pt(6),
                    pb(4),
                    flex_none(),
                ])
                .child(settings_servers::page_title(section)),
        )
        .child(settings_servers::body(state, section, room))
}
