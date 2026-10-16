//! The settings panel: a modal with the sections of settings down its left side and the one chosen on its right.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, bg, border_color, border_r, flex, flex_1, flex_col, flex_none, gap, h, h_px,
    items_center, justify_end, min_w_px, p, pl, px, w_px,
};
use pm_ui::{Node, node};

use crate::button::{Size, Variant};
use crate::parts::{glyph, line};
use crate::settings_servers;
use crate::state::WorkspaceState;
use crate::tile::Tile;
use crate::{Button, Modal};

/// Closes the panel, which is what pressing outside it does.
pub const SETTINGS_SCRIM: FocusId = FocusId(20_000_000);

/// The panel itself, which keeps a press on it from counting as outside.
pub const SETTINGS_BODY: FocusId = FocusId(20_000_001);

/// Closes the panel.
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

/// The first of the sections down the left side; the rest follow it.
pub const SETTINGS_SECTION_BASE: u64 = 20_000_010;

/// The first of the buttons editing a server of the list showing; the rest follow it.
pub const SETTINGS_EDIT_BASE: u64 = 20_000_100;

/// The first of the buttons removing a server of the list showing; the rest follow it.
pub const SETTINGS_REMOVE_BASE: u64 = 20_000_200;

/// The first of the buttons installing what a registry offers; the rest follow it.
pub const SETTINGS_INSTALL_BASE: u64 = 20_000_300;

/// One past the last name the panel answers to.
const SETTINGS_END: u64 = 20_001_500;

/// How far the panel keeps from every edge of the window, in logical pixels.
const MARGIN: f32 = 14.0;

/// How much of the window's width the panel takes, at least as wide as [`MIN_WIDTH`] where the window has room.
const WIDTH_SHARE: f32 = 0.8;

/// How much of the window's height the panel takes, at least as tall as [`MIN_HEIGHT`] where the window has room.
const HEIGHT_SHARE: f32 = 0.85;

/// The least wide the panel is made, in logical pixels, in a window that has room for it.
const MIN_WIDTH: f32 = 1240.0;

/// The least tall the panel is made, in logical pixels, in a window that has room for it.
const MIN_HEIGHT: f32 = 780.0;

/// `share` of a window `window` logical pixels across, no less than `least` and never past the margins.
fn panel_extent(window: f32, share: f32, least: f32) -> f32 {
    (window * share).max(least).min(window - 2.0 * MARGIN)
}

/// How wide the list of sections is, in logical pixels.
const NAVIGATION_WIDTH: f32 = 220.0;

/// A group of settings, named down the left side of the panel; some sit inside another.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Section {
    /// How the window looks and behaves.
    #[default]
    General,
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
    /// What agents are started with, with the groups below it.
    Agents,
    /// The agents that can be started, and the ones added by hand.
    AgentServers,
    /// The tool servers every agent is started with.
    McpServers,
}

impl Section {
    /// Every section, in the order the list shows them, each followed by the ones inside it.
    pub const ALL: [Section; 10] = [
        Section::General,
        Section::Editor,
        Section::EditorAppearance,
        Section::EditorEditing,
        Section::EditorSaving,
        Section::EditorIntelligence,
        Section::EditorSearch,
        Section::Agents,
        Section::AgentServers,
        Section::McpServers,
    ];

    /// What the list and the heading call it.
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Editor => "Editor",
            Self::EditorAppearance => "Appearance",
            Self::EditorEditing => "Editing",
            Self::EditorSaving => "Saving",
            Self::EditorIntelligence => "Code Intelligence",
            Self::EditorSearch => "Search & Scrolling",
            Self::Agents => "Agents",
            Self::AgentServers => "Agent Servers",
            Self::McpServers => "MCP Servers",
        }
    }

    /// The glyph before its name in the list.
    fn icon(self) -> IconName {
        match self {
            Self::General => IconName::Settings,
            Self::Editor
            | Self::EditorAppearance
            | Self::EditorEditing
            | Self::EditorSaving
            | Self::EditorIntelligence
            | Self::EditorSearch => IconName::FileCode,
            Self::Agents => IconName::Sparkles,
            Self::AgentServers => IconName::Folder,
            Self::McpServers => IconName::Plug,
        }
    }

    /// The section it sits inside, if it sits inside one.
    pub(crate) fn parent(self) -> Option<Section> {
        match self {
            Self::AgentServers | Self::McpServers => Some(Self::Agents),
            Self::EditorAppearance
            | Self::EditorEditing
            | Self::EditorSaving
            | Self::EditorIntelligence
            | Self::EditorSearch => Some(Self::Editor),
            Self::General | Self::Agents | Self::Editor => None,
        }
    }

    /// The name of pressing its row in the list.
    pub fn id(self) -> FocusId {
        let position = Self::ALL.iter().position(|section| *section == self);
        FocusId(SETTINGS_SECTION_BASE + position.unwrap_or_default() as u64)
    }
}

/// Whether `id` names something of the panel.
pub fn is_settings(id: FocusId) -> bool {
    let FocusId(number) = id;
    (SETTINGS_SCRIM.0..SETTINGS_END).contains(&number)
}

impl WorkspaceState {
    /// Whether the settings panel is open.
    pub fn settings_open(&self) -> bool {
        self.settings.is_some()
    }

    /// The state with the settings panel open on its first section.
    pub(crate) fn with_settings(mut self) -> Self {
        self.menu = None;
        self.picker = None;
        self.settings = Some(Section::default());
        self.settings_servers = settings_servers::ServerSettings::opening(
            std::mem::take(&mut self.settings_servers.agents),
            std::mem::take(&mut self.settings_servers.retired),
        );
        self.outbox.push(crate::state::Outgoing::LoadServers);
        self
    }

    /// The state after something of the settings panel was pressed: `action` is the name of what.
    pub(crate) fn settings_pressed(mut self, action: FocusId) -> Self {
        match action {
            SETTINGS_CLOSE if self.settings_servers.form.is_some() => {
                self.settings_servers.form = None;
            }
            SETTINGS_SCRIM | SETTINGS_CLOSE => self.settings = None,
            action if self.editor_setting_pressed(action) => {}
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
                    self.section_changed();
                } else {
                    return self.servers_pressed(action);
                }
            }
        }
        self
    }
}

/// The node that draws the settings panel, when it is open, sized from a window `width` by `height` logical pixels.
pub fn settings(state: &WorkspaceState, width: f32, height: f32) -> Option<Node> {
    let section = state.settings?;
    let folded = state.settings_servers.folded;
    Some(
        Modal::new(SETTINGS_SCRIM, SETTINGS_BODY)
            .width(panel_extent(width, WIDTH_SHARE, MIN_WIDTH))
            .child(
                node()
                    .styles([flex(), h_px(panel_extent(height, HEIGHT_SHARE, MIN_HEIGHT))])
                    .child(navigation(section, folded))
                    .child(content(state, section)),
            )
            .build(),
    )
}

/// The list of sections, with the one showing marked.
fn navigation(showing: Section, folded: bool) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            gap(1),
            p(3),
            w_px(NAVIGATION_WIDTH),
            border_r(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([flex(), items_center(), h(9), px(2)])
                .child(line("Settings", 13, 1.4, Role::Subtle)),
        )
        .children(
            Section::ALL
                .into_iter()
                .filter(|section| match section.parent() {
                    Some(Section::Agents) => !folded,
                    Some(parent) => showing == parent || showing.parent() == Some(parent),
                    None => true,
                })
                .map(|section| entry(section, section == showing, folded)),
        )
}

/// One row of the list of sections: General has its glyph, Agents the chevron that says whether the sections inside it are shown, and those inside it only their name, set in under it.
fn entry(section: Section, showing: bool, folded: bool) -> Node {
    let tone = if showing { Role::Text } else { Role::Muted };
    let row = Tile::new(section.id())
        .selected(showing)
        .build()
        .styles([px(2)]);
    let row = match section {
        Section::General | Section::Editor => row.child(glyph(section.icon(), 4.0, tone)),
        Section::Agents => row.child(glyph(
            if folded {
                IconName::ChevronRight
            } else {
                IconName::ChevronDown
            },
            4.0,
            tone,
        )),
        _ => row.styles([pl(8)]),
    };
    row.child(line(section.title(), 13, 1.4, tone))
}

/// The settings of `section` under the button that closes the panel.
fn content(state: &WorkspaceState, section: Section) -> Node {
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
                    justify_end(),
                    h(12),
                    px(4),
                    flex_none(),
                ])
                .child(
                    Button::icon(IconName::Close, SETTINGS_CLOSE)
                        .variant(Variant::Ghost)
                        .size(Size::Icon)
                        .build(),
                ),
        )
        .child(settings_servers::body(state, section))
}
